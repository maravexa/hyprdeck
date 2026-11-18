"""Check capture and pointer confinement separately on a disposable desktop."""

from contextlib import ExitStack
import json
import os
from pathlib import Path
import re
import subprocess
import sys
import time

from journal_restart import rpc


def window(name):
    import gi

    gi.require_version("Gtk", "4.0")
    gi.require_version("Gdk", "4.0")
    from gi.repository import Gdk, Gio, Gtk

    def activate(app):
        color = {"A": "#357a57", "B": "#a64148"}[name]
        styles = Gtk.CssProvider()
        styles.load_from_data(f"button {{background-image:none; background-color:{color};}}".encode())
        Gtk.StyleContext.add_provider_for_display(Gdk.Display.get_default(), styles, 800)
        surface = Gtk.ApplicationWindow(application=app, title=f"HyprDeck Scope {name}")
        surface.set_decorated(False)
        surface.set_default_size(360, 240)
        button = Gtk.Button(label=f"Fixture {name}")
        button.connect("clicked", lambda _: print(f"click-{name}", flush=True))
        motion = Gtk.EventControllerMotion()
        motion.connect("enter", lambda *_: print(f"enter-{name}", flush=True))
        button.add_controller(motion)
        surface.set_child(button)
        surface.present()

    app = Gtk.Application(application_id=f"org.example.HyprDeckScope{name}", flags=Gio.ApplicationFlags.NON_UNIQUE)
    app.connect("activate", activate)
    return app.run([])


def wait_for(description, predicate):
    deadline = time.monotonic() + 5
    while time.monotonic() < deadline:
        value = predicate()
        if value:
            return value
        time.sleep(0.04)
    raise TimeoutError(description)


def command(*args):
    return subprocess.run(args, check=True, timeout=4, capture_output=True).stdout


def dispatch(verb, argument):
    result = command("hyprctl", "dispatch", verb, argument).strip()
    assert result == b"ok", (verb, result)


def clients():
    return {row["title"]: row for row in json.loads(command("hyprctl", "-j", "clients"))}


def capture_window(evidence):
    globals_text = command("wayland-info").decode()
    relevant = [line for line in globals_text.splitlines()
                if any(kind in line for kind in ("toplevel", "image_capture", "image_copy", "virtual_pointer"))]
    (evidence / "protocols.txt").write_text("\n".join(relevant) + "\n")
    env = os.environ | {"WAYLAND_DEBUG": "client"}
    trace = subprocess.run(["grim", "-t", "png", "-T", "missing-test-id", "-"], env=env,
                           capture_output=True, timeout=5).stderr.decode()
    (evidence / "capture-probe.txt").write_text(trace)
    handles = dict(re.findall(r'(ext_foreign_toplevel_handle_v1[@#]\d+)\.identifier\("([^"]+)"\)', trace))
    title = re.search(r'(ext_foreign_toplevel_handle_v1[@#]\d+)\.title\("HyprDeck Scope A"\)', trace)
    if not title or title[1] not in handles:
        return {"proved": False, "reason": trace.splitlines()[-1] if trace else "No capturable foreign toplevel"}, None
    identifier = handles[title[1]]
    result = subprocess.run(["grim", "-t", "png", "-T", identifier, "-"], timeout=5, capture_output=True)
    if result.returncode:
        return {"proved": False, "reason": result.stderr.decode().strip()}, None
    data = result.stdout
    image = evidence / "window-a.png"
    image.write_bytes(data)
    width = int.from_bytes(data[16:20], "big")
    height = int.from_bytes(data[20:24], "big")
    assert (width, height) == (360, 240), (width, height)
    # Compare the overlapped window's mean color to the known green fixture.
    pixels = command("magick", image, "-crop", "16x16+10+10", "-scale", "1x1!", "-depth", "8", "rgb:-")
    assert pixels == bytes.fromhex("357a57"), list(pixels)
    return {"proved": True, "size": [width, height], "rgb": list(pixels)}, identifier


def probe(pointer, driver, evidence):
    runtime = Path(os.environ["XDG_RUNTIME_DIR"])
    assert runtime.parent.parent == Path("/tmp") and runtime.parent.name.startswith("hd.")
    assert (runtime / "proof-pointer").is_file()
    display = runtime / os.environ["WAYLAND_DISPLAY"]
    with ExitStack() as stack:
        def launch(args, name, protocol=False):
            log = stack.enter_context((evidence / f"{name}.log").open("w"))
            process = subprocess.Popen(args, stdin=subprocess.PIPE if protocol else None,
                                       stdout=subprocess.PIPE if protocol else log, stderr=log, text=True)

            def stop():
                if process.poll() is None:
                    process.terminate()
                    try:
                        process.wait(timeout=2)
                    except subprocess.TimeoutExpired:
                        process.kill()
                        process.wait(timeout=2)

            stack.callback(stop)
            return process

        first_window = launch([sys.executable, "-B", __file__, "--window", "A"], "A")
        launch([sys.executable, "-B", __file__, "--window", "B"], "B")
        windows = wait_for("two mapped benign Wayland windows", lambda: (rows if len(rows := clients()) == 2 else None))
        a, b = ("address:" + windows[f"HyprDeck Scope {name}"]["address"] for name in ("A", "B"))
        assert all(not row["xwayland"] for row in windows.values())
        for address, coordinates in [(a, "230 180"), (b, "640 180")]:
            dispatch("setfloating", address)
            dispatch("resizewindowpixel", f"exact 360 240,{address}")
            dispatch("movewindowpixel", f"exact {coordinates},{address}")
        dispatch("focuswindow", a)
        x, y = wait_for("mapped fixture A", lambda: clients()["HyprDeck Scope A"]["at"])
        x, y = x + 90, y + 60
        policy = launch([driver], "policy", protocol=True)
        seat = launch([pointer, display, str(x), str(y)], "pointer", protocol=True)

        def check(resource=1, revision=0, operation="activate"):
            return rpc(policy, {"type": "check", "request": {"binding": {"runtime": 1, "task": 1, "resource": resource},
                                                             "revision": revision, "operation": operation, "destination": "local"}})["denial"]

        def receipts(name, event="click"):
            return (evidence / f"{name}.log").read_text().splitlines().count(f"{event}-{name}")

        def send_input(control):
            seat.stdin.write(control)
            seat.stdin.flush()

        def wait_for_cursor(px, py):
            def cursor():
                position = json.loads(command("hyprctl", "-j", "cursorpos"))
                return abs(position["x"] - px) <= 1 and abs(position["y"] - py) <= 1
            wait_for(f"fixture pointer reached {px},{py}", cursor)

        def click(expected):
            send_input("r")
            wait_for_cursor(30, 30)
            entered, clicked = receipts(expected, "enter"), receipts(expected)
            send_input("m")
            wait_for_cursor(x, y)
            wait_for(f"fixture pointer entered {expected}", lambda: receipts(expected, "enter") > entered)
            send_input("c")
            wait_for(f"fixture pointer clicked {expected}", lambda: receipts(expected) == clicked + 1)

        assert check() is None
        click("A")
        assert receipts("B") == 0
        dispatch("movewindowpixel", f"exact {x - 90} {y - 60},{b}")
        dispatch("focuswindow", b)
        dispatch("alterzorder", f"top,{b}")
        wait_for("B has focus over A", lambda: json.loads(command("hyprctl", "-j", "activewindow"))["title"] == "HyprDeck Scope B")
        assert clients()["HyprDeck Scope A"]["at"] == clients()["HyprDeck Scope B"]["at"]
        clip = f"{x},{y} 1x1"
        wait_for("B visibly covers A at the test point",
                 lambda: command("grim", "-g", clip, "-t", "ppm", "-").endswith(bytes.fromhex("a64148")))
        revision = rpc(policy, {"type": "advance"})["revision"]
        assert check(revision=revision, operation="observe_visual") is None
        assert check(resource=2, revision=revision, operation="observe_visual") == "wrong_binding"
        capture_result, identifier = capture_window(evidence)
        outcome = {"window_capture": capture_result, "unchanged_click_coordinates": [x, y]}
        (evidence / "overlap.png").write_bytes(command("grim", "-t", "png", "-"))

        assert check() == "stale_revision"
        assert check(resource=2, revision=revision) == "wrong_binding"
        rpc(policy, {"type": "revoke"})
        assert check(revision=revision) == "revoked"
        assert [receipts("A"), receipts("B")] == [1, 0]
        outcome["policy_before_unscoped_control"] = {"A": 1, "B": 0, "denials": ["stale_revision", "wrong_binding", "revoked"]}

        # Intentional negative control against disposable B. This tests the
        # compositor protocol by omitting the policy for one benign click.
        click("B")
        assert receipts("A") == 1
        seat.stdin.close()
        assert seat.wait(timeout=5) == 0
        outcome["unscoped_pointer_recipient"] = "B"
        outcome["window_only_native_input"] = False
        if identifier:
            first_window.terminate()
            first_window.wait(timeout=5)
            wait_for("original A closed", lambda: "HyprDeck Scope A" not in clients())
            launch([sys.executable, "-B", __file__, "--window", "A"], "replacement-A")
            wait_for("new A with the same title", lambda: "HyprDeck Scope A" in clients())
            stale = subprocess.run(["grim", "-T", identifier, "-"], timeout=5, capture_output=True)
            assert stale.returncode != 0 and b"cannot find toplevel" in stale.stderr, stale.stderr
            outcome["window_capture"]["old_id_rejected_after_replacement"] = True
        (evidence / "scope-result.json").write_text(json.dumps(outcome, indent=2) + "\n")
        print(json.dumps(outcome))


if __name__ == "__main__":
    if sys.argv[1] == "--window":
        sys.exit(window(sys.argv[2]))
    probe(Path(sys.argv[1]), Path(sys.argv[2]), Path(sys.argv[3]))
