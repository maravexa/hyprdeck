"""Exercise the normal HyprDeck binary on a disposable nested compositor."""

import argparse
import hashlib
import json
import os
from pathlib import Path
import signal
import socket
import subprocess
import tempfile
import time
from xml.sax.saxutils import escape


COMPOSITOR = """\
monitor = , 1024x768@60, auto, 1
workspace = 1, persistent:true, default:true
workspace = 2, persistent:true
xwayland {
    enabled = false
}
misc {
    disable_hyprland_logo = true
    disable_splash_rendering = true
}
animations {
    enabled = false
}
debug {
    disable_logs = false
}
"""

THEME = """\
name = "Workspace proof"
description = "Disposable panel with two persistent workspaces"
[[panels]]
edge = "top"
height = 40
layout = "horizontal"
auto_hide = { mode = "disabled" }
modules_center = ["workspaces"]
[panels.module_styles.workspaces]
active_background = "#2680eb"
inactive_background = "#707080"
[style]
background_color = "#242833"
font_family = "DejaVu Sans"
"""


def wait_for(label, predicate, children=(), timeout=10):
    end = time.monotonic() + timeout
    while time.monotonic() < end:
        for child in children:
            if child.poll() is not None:
                raise RuntimeError(f"{child.args[0]} exited ({child.returncode}) while waiting for {label}")
        result = predicate()
        if result:
            return result
        time.sleep(0.08)
    raise TimeoutError(f"Timed out waiting for {label}")


def color_center(ppm, color):
    header, dimensions, depth, pixels = ppm.split(b"\n", 3)
    assert header == b"P6" and depth == b"255"
    width, height = map(int, dimensions.split())
    assert (width, height) == (1024, 768), dimensions
    row = pixels[10 * width * 3:11 * width * 3]
    rgb = bytes.fromhex(color)
    matches = [x for x in range(width) if row[x * 3:x * 3 + 3] == rgb]
    return matches[len(matches) // 2] if matches else None


def run(binary, pointer_fixture, evidence, scratch, window_scope):
    repo = Path(__file__).resolve().parents[1]
    runtime = scratch / "r"
    config = scratch / "config"
    for directory in (runtime, config, scratch / "data", scratch / "cache", scratch / "state"):
        directory.mkdir(mode=0o700)
    with socket.socket(socket.AF_UNIX) as probe:
        # Reject an overlong socket path before starting the desktop.
        probe.bind(str(runtime / "bus"))
    (runtime / "bus").unlink()
    (runtime / "proof-pointer").touch(mode=0o600)
    env = {
        "PATH": os.environ["PATH"], "LANG": "C.UTF-8", "XDG_RUNTIME_DIR": str(runtime),
        "XDG_CONFIG_HOME": str(config), "XDG_DATA_HOME": str(scratch / "data"),
        "XDG_CACHE_HOME": str(scratch / "cache"), "XDG_STATE_HOME": str(scratch / "state"),
        "GSETTINGS_BACKEND": "memory", "HYPRLAND_NO_SD_VARS": "1", "RUST_LOG": "hyprdeck=info",
        "DBUS_SESSION_BUS_ADDRESS": f"unix:path={runtime / 'bus'}",
    }
    font = repo / "crates/hyprdeck-core/tests/fixtures"
    font_config = scratch / "fonts.conf"
    font_config.write_text(f'<fontconfig><dir>{escape(str(font))}</dir>'
                           f'<cachedir>{scratch / "cache"}</cachedir></fontconfig>\n')
    env["FONTCONFIG_FILE"] = str(font_config)
    (config / "hypr").mkdir()
    compositor = config / "hypr/hyprland.conf"
    compositor.write_text(COMPOSITOR)
    (config / "hypr/hyprdeck.toml").write_text('theme = "proof"\n')
    theme = config / "hyprdeck/themes/proof/theme.toml"
    theme.parent.mkdir(parents=True)
    theme.write_text(THEME)
    (evidence / "theme-before.toml").write_text(THEME)
    children = []

    def launch(log, *args, stdin=None, **extra_env):
        with (evidence / log).open("wb") as output:
            process = subprocess.Popen(args, env=env | extra_env, stdin=stdin, stdout=output, stderr=output,
                                       start_new_session=True)
        children.append(process)
        return process

    def command(*args):
        return subprocess.check_output(args, env=env, timeout=4, stderr=subprocess.PIPE)

    def check_pixel(color):
        try:
            return color_center(command("grim", "-t", "ppm", "-"), color)
        except subprocess.TimeoutExpired:
            return None

    try:
        bus = launch("bus.log", "dbus-daemon", "--session", "--nofork", f"--address={env['DBUS_SESSION_BUS_ADDRESS']}")
        wait_for("private D-Bus", (runtime / "bus").is_socket, (bus,))
        kwin = launch("kwin.log", "kwin_wayland", "--virtual", "--socket", "wayland-parent", "--no-lockscreen",
                      "--no-global-shortcuts", "--no-kactivities", "--width", "1024", "--height", "768")
        wait_for("virtual KWin socket", (runtime / "wayland-parent").is_socket, (kwin,))
        hypr = launch("hyprland.log", "Hyprland", "--config", str(compositor), WAYLAND_DISPLAY="wayland-parent")

        def instance():
            for lock in (runtime / "hypr").glob("*/hyprland.lock"):
                fields = lock.read_text().splitlines()
                if len(fields) == 2 and fields[0] == str(hypr.pid) and (runtime / fields[1]).is_socket():
                    return lock.parent.name, fields[1]
            return None

        signature, display = wait_for("nested Hyprland socket", instance, (hypr,))
        env.update(HYPRLAND_INSTANCE_SIGNATURE=signature, WAYLAND_DISPLAY=display)

        def monitors():
            try:
                return json.loads(command("hyprctl", "-j", "monitors"))
            except subprocess.CalledProcessError:
                return None

        monitor, = wait_for("nested monitor", monitors, (hypr,))
        assert (monitor["width"], monitor["height"], monitor["scale"]) == (1024, 768, 1)
        command("hyprctl", "dispatch", "workspace", "1")
        if window_scope:
            worker = Path(__file__).with_name("scoped_windows.py")
            scope = launch("scope-worker.log", "python3", "-B", worker, pointer_fixture, window_scope, evidence)
            if scope.wait(timeout=55) != 0:
                raise RuntimeError("Window-scope fixture failed. See scope-worker.log.")
            return
        deck = launch("hyprdeck.log", str(binary))
        deck_log = evidence / "hyprdeck.log"
        wait_for("HyprDeck startup", lambda: b"Entering main event loop" in deck_log.read_bytes(), (deck, hypr))
        first_x = wait_for("original blue workspace", lambda: check_pixel("2680eb"), (deck, hypr))
        (evidence / "before.png").write_bytes(command("grim", "-t", "png", "-"))

        start = time.monotonic()
        patched = THEME.replace("#2680eb", "#ec8534")
        theme.write_text(patched)
        (evidence / "theme-after.toml").write_text(patched)
        command(str(binary))  # Normal second invocation requests a reload.
        wait_for("patched orange workspace", lambda: check_pixel("ec8534") == first_x, (deck, hypr))
        patch_ms = round((time.monotonic() - start) * 1000)
        (evidence / "after-patch.png").write_bytes(command("grim", "-t", "png", "-"))

        second_x = check_pixel("707080")
        assert second_x is not None and second_x > first_x, "Workspace 2 is missing from the patched frame"
        pointer = launch("pointer.log", str(pointer_fixture), str(runtime / display), str(second_x),
                         stdin=subprocess.PIPE)
        wait_for("panel pointer registration",
                 lambda: b"SEAT Created pointer" in deck_log.read_bytes(),
                 (deck, hypr, pointer))
        pointer.stdin.write(b"m")
        pointer.stdin.flush()

        def cursor_on_button():
            cursor = json.loads(command("hyprctl", "-j", "cursorpos"))
            return abs(cursor["x"] - second_x) <= 1 and abs(cursor["y"] - 16) <= 1

        wait_for("fixture pointer on workspace button", cursor_on_button, (deck, hypr, pointer))
        with socket.socket(socket.AF_UNIX) as events:
            events.settimeout(5)
            events.connect(str(runtime / "hypr" / signature / ".socket2.sock"))
            click_start = time.monotonic()
            pointer.stdin.write(b"c")
            pointer.stdin.flush()
            with events.makefile("rb") as event_lines:
                for line in event_lines:
                    if line.startswith(b"workspacev2>>2,"):
                        break
                else:
                    raise RuntimeError("Compositor event socket closed before workspace 2")
        active = json.loads(command("hyprctl", "-j", "activeworkspace"))
        assert active["id"] == 2, active
        wait_for("orange paint on workspace 2", lambda: check_pixel("ec8534") == second_x, (deck, hypr))
        click_ms = round((time.monotonic() - click_start) * 1000)
        (evidence / "after-click.png").write_bytes(command("grim", "-t", "png", "-"))
        build = subprocess.check_output(["git", "-C", str(repo), "rev-parse", "HEAD"], text=True).strip()
        with binary.open("rb") as binary_file:
            binary_digest = hashlib.file_digest(binary_file, "sha256").hexdigest()
        result = {"build_revision": build, "binary_sha256": binary_digest,
                  "resource": monitor["name"], "width": 1024, "height": 768, "scale": 1,
                  "workspace_after_click": active["id"], "baseline_x": first_x, "clicked_x": second_x,
                  "patch_to_frame_ms": patch_ms, "click_to_frame_ms": click_ms}
        (evidence / "result.json").write_text(json.dumps(result, indent=2) + "\n")
        print(json.dumps(result))
    except (OSError, AssertionError, subprocess.SubprocessError, RuntimeError) as error:
        (evidence / "failure.txt").write_text(f"{type(error).__name__}: {error}\n")
        raise
    finally:
        for child in reversed(children):
            try:
                os.killpg(child.pid, signal.SIGTERM)
            except ProcessLookupError:
                pass
            try:
                child.wait(timeout=3)
            except subprocess.TimeoutExpired:
                pass
            try:
                os.killpg(child.pid, signal.SIGKILL)
            except ProcessLookupError:
                pass
            child.wait(timeout=3)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("binary", type=Path)
    parser.add_argument("pointer_fixture", type=Path)
    parser.add_argument("new_evidence_directory", type=Path)
    parser.add_argument("--window-scope", type=Path, help="run the two-window fixture with this scope policy driver")
    args = parser.parse_args()
    evidence = args.new_evidence_directory.resolve()
    evidence.mkdir(mode=0o700)
    with tempfile.TemporaryDirectory(prefix="hd.", dir="/tmp") as scratch:
        scope = args.window_scope.resolve(strict=True) if args.window_scope else None
        run(args.binary.resolve(strict=True), args.pointer_fixture.resolve(strict=True), evidence, Path(scratch), scope)


if __name__ == "__main__":
    main()
