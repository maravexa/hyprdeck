//! Shared wordlists for pope, oracle, and koan subcommands.
//!
//! All lists are `pub const &[&str]` so they can be indexed by a
//! deterministic hash at runtime without allocation.

// ─── Pope ──────────────────────────────────────────────────────────────────

pub const POPE_HONORIFICS: &[&str] = &[
    "His Holiness",
    "Her Holiness",
    "Their Holiness",
    "The Most Illuminated",
    "The Irreverend",
    "The Chaotic",
    "The Sublime",
    "His Fraudulency",
    "Her Incoherence",
    "Pope",
    "Antipope",
    "Archpope",
    "The Discordant",
];

pub const POPE_ADJECTIVES: &[&str] = &[
    "Golden",
    "Sacred",
    "Flaxen",
    "Twisted",
    "Erisian",
    "Confused",
    "Sublime",
    "Bureaucratic",
    "Forgotten",
    "Radiant",
    "Pestilent",
    "Unknowable",
    "Suspicious",
    "Gloriously Redundant",
    "Fundamentally Unsound",
];

pub const POPE_NOUNS: &[&str] = &[
    "Apple",
    "Cabbage",
    "Fnord",
    "Hotdog",
    "Chao",
    "Flax",
    "Pineal Gland",
    "Discordia",
    "Pagoda",
    "Mandible",
    "Vexation",
    "Whatnot",
    "Kallisti",
    "Turnip",
    "Wossname",
];

pub const SECT_ADJECTIVES: &[&str] = &[
    "Erisian",
    "Discordant",
    "Illuminated",
    "Confused",
    "Sublime",
    "Forgotten",
    "Chaotic",
    "Sacred",
    "Twisted",
    "Bureaucratic",
    "Radiant",
    "Suspicious",
];

pub const SECT_NOUNS: &[&str] = &[
    "Cabal",
    "Order",
    "Assembly",
    "Pagoda",
    "Enclave",
    "Sect",
    "Syndicate",
    "Collective",
    "Anomaly",
    "Concordance",
    "Hotdog Stand",
    "Fnord",
];

pub const PAPAL_DECREES: &[&str] = &[
    "All hotdogs are sacred. All hotdogs are also not hotdogs.",
    "The Law of Fives is always in effect, especially when it isn't.",
    "Bureaucracy is hereby disbanded. Please file form 23-B to confirm.",
    "Fnord.",
    "All Popes are equal. Some Popes are more equal than others, but we don't talk about that.",
    "It is decreed that everything is permitted, especially confusion.",
    "The Sacred Chao shall be honored by doing something weird on Pungenday.",
    "Cabbage is legal tender in all Discordian transactions.",
    "You are already enlightened. You just forgot.",
    "This decree is intentionally left blank.",
    "Flax shall be weighed by the ton, and only by the ton.",
    "Greyface is hereby excommunicated from whatever it is we have.",
    "Every fifth word in every fifth sentence is a fnord. You cannot see them.",
    "The Pineal Gland is the only organ with Papal Infallibility.",
    "All meetings shall begin with five minutes of confusion.",
    "Order is forbidden. Also mandatory. Consult your local Pope.",
    "The Apple of Discord is not for eating. Or is it?",
    "Any hotdog eaten after midnight becomes a sandwich. Discuss.",
    "No Discordian shall be required to believe anything, including this.",
    "The Pope shall receive one (1) cabbage upon coronation. No more, no less.",
    "Laughter is the only sacrament. Snorting counts.",
    "Chaos is not the enemy of order. Order is the enemy of order.",
    "The number 23 is sacred. So is 17. And 42. And pretty much all of them.",
    "To achieve enlightenment, stare at a fnord until you stop seeing it.",
    "The Discordian Calendar is correct. All other calendars are suggestions.",
];

// ─── Oracle ────────────────────────────────────────────────────────────────

pub const ORACLE_OPENINGS: &[&str] = &[
    "The Sacred Chao reveals that",
    "Eris whispers:",
    "The Law of Fives indicates that",
    "It is written in flax that",
    "The Oracle has consulted the golden apple and determined that",
    "Fnord. Also,",
    "The Principia is clear on this matter:",
    "After careful divination,",
    "The answer, as always, is related to cabbage. Specifically,",
    "The pineal gland tingles. This means",
];

/// Oracle middles — `{n}` is replaced with the count of '5' or 'f'
/// characters in the question.
pub const ORACLE_MIDDLES: &[&str] = &[
    "your query contains {n} fives,",
    "the answer is both yes and no, simultaneously,",
    "you already know the answer,",
    "the question itself is the answer,",
    "a hotdog is involved somehow,",
    "this is entirely Greyface's fault,",
    "the Sacred Chao is in balance,",
    "discord is required,",
    "order is the problem,",
    "fnord fnord fnord,",
];

pub const ORACLE_CLOSINGS: &[&str] = &[
    "and you should act accordingly.",
    "Hail Eris.",
    "especially on Pungenday.",
    "but only if you believe in the Law of Fives.",
    "consult your local Pope for clarification.",
    "the rest is up to you.",
    "this has always been true.",
    "further divination is not recommended.",
    "do not think about this too hard.",
    "fnord.",
];

// ─── Koan ──────────────────────────────────────────────────────────────────

pub const KOAN_SETUPS: &[&str] = &[
    "A student asked the Pope:",
    "Malaclypse the Younger once said to a hotdog vendor:",
    "In the 73rd year of Our Lady of Discord, a confused bureaucrat asked:",
    "The Sacred Chao was spinning when someone inquired:",
    "A seeker of fnords approached the Oracle and said:",
    "On a Pungenday in the season of Confusion,",
    "After filing form 23-B in triplicate,",
    "Greyface appeared in a dream and demanded to know:",
    "The Law of Fives was invoked when a student asked:",
    "At the Discordian Council (quorum: one Pope),",
    "A novice, clutching five cabbages, said to her teacher:",
    "The Goddess of Discord leaned down from the clouds and inquired:",
    "During the Aftermath of a particularly loud Pungenday,",
    "An Erisian monk dropped his flax and asked aloud:",
    "While lost in the Pagoda of Unknowing, a traveler asked:",
];

pub const KOAN_QUESTIONS: &[&str] = &[
    "'What is the sound of five tons of flax?'",
    "'Is a hotdog a sandwich?'",
    "'If a fnord falls in a forest and no one reads it, is it still there?'",
    "'What is the weight of bureaucracy?'",
    "'How many Popes does it take to confuse a light bulb?'",
    "'Where does the Sacred Chao go when no one is looking?'",
    "'Is order the enemy of chaos, or just confused about it?'",
    "'What color is the Law of Fives?'",
    "'If Eris threw the apple today, who would catch it?'",
    "'Why is a cabbage?'",
    "'Does the Pope dream of electric hotdogs?'",
    "'What lies on the other side of the golden apple?'",
    "'Can a bureaucrat enlighten a turnip?'",
    "'Is there a form for becoming a Pope, and is it in triplicate?'",
    "'What is the chao-per-fnord ratio at 3 AM?'",
];

pub const KOAN_RESPONSES: &[&str] = &[
    "The Pope considered this for five days, then replied: 'Fnord.'",
    "Eris laughed. The student was enlightened.",
    "The answer was filed under 'K' for 'Kallisti' and never retrieved.",
    "A hotdog appeared. No further questions were asked.",
    "The Sacred Chao spun counterclockwise. This was considered sufficient.",
    "The Oracle said: 'You already know.' The student did not know.",
    "Five bureaucrats were consulted. None agreed. All were correct.",
    "The Pope replied: 'That is not a question. Neither is this an answer.'",
    "Greyface frowned. This was taken as a good sign.",
    "The questioner became a Pope. This solved nothing and everything.",
    "A cabbage rolled past. The student took this as an answer.",
    "The Goddess whispered only: 'Five.' She then vanished into a hotdog.",
    "The teacher threw a fnord at the student. The student dodged.",
    "All present agreed to disagree, and then disagreed about the agreement.",
    "The answer was on the back of form 23-B, which was, of course, missing.",
];

pub const KOAN_REFLECTIONS: &[&str] = &[
    "Contemplate this in light of: \"{input}\"",
    "The Oracle notes: \"{input}\". Meditate accordingly.",
    "Your words — \"{input}\" — have been weighed. They weigh 5 tons of flax.",
    "This was written: \"{input}\". The Sacred Chao is unmoved.",
    "Eris read \"{input}\" and said only: 'Yes.'",
];
