use crate::date::types::Season;
use crate::holydays::types::{Holyday, HolydayKey, HolydaySource};

pub fn builtin_holydays() -> Vec<Holyday> {
    vec![
        // Apostle Holydays (day 5 of each season)
        Holyday {
            key: HolydayKey::SeasonDay {
                season: Season::Chaos,
                day: 5,
            },
            name: "Mungday".to_string(),
            description: Some(
                "The feast day of Hung Mung, patron apostle of the Season of Chaos.".to_string(),
            ),
            greeting: Some("Hail Eris! Happy Mungday!".to_string()),
            apostle: Some("Hung Mung".to_string()),
            recurring: true,
            year: None,
            source: HolydaySource::Default,
        },
        Holyday {
            key: HolydayKey::SeasonDay {
                season: Season::Discord,
                day: 5,
            },
            name: "Mojoday".to_string(),
            description: Some(
                "The feast day of Dr. Van Van Mojo, patron apostle of the Season of Discord."
                    .to_string(),
            ),
            greeting: Some("Hail Eris! Happy Mojoday!".to_string()),
            apostle: Some("Dr. Van Van Mojo".to_string()),
            recurring: true,
            year: None,
            source: HolydaySource::Default,
        },
        Holyday {
            key: HolydayKey::SeasonDay {
                season: Season::Confusion,
                day: 5,
            },
            name: "Syaday".to_string(),
            description: Some(
                "The feast day of Sri Syadasti, patron apostle of the Season of Confusion."
                    .to_string(),
            ),
            greeting: Some("Hail Eris! Happy Syaday!".to_string()),
            apostle: Some("Sri Syadasti".to_string()),
            recurring: true,
            year: None,
            source: HolydaySource::Default,
        },
        Holyday {
            key: HolydayKey::SeasonDay {
                season: Season::Bureaucracy,
                day: 5,
            },
            name: "Zaraday".to_string(),
            description: Some(
                "The feast day of Zarathud, patron apostle of the Season of Bureaucracy."
                    .to_string(),
            ),
            greeting: Some("Hail Eris! Happy Zaraday!".to_string()),
            apostle: Some("Zarathud".to_string()),
            recurring: true,
            year: None,
            source: HolydaySource::Default,
        },
        Holyday {
            key: HolydayKey::SeasonDay {
                season: Season::Aftermath,
                day: 5,
            },
            name: "Maladay".to_string(),
            description: Some(
                "The feast day of The Elder Malaclypse, patron apostle of the Season of Aftermath."
                    .to_string(),
            ),
            greeting: Some("Hail Eris! Happy Maladay!".to_string()),
            apostle: Some("The Elder Malaclypse".to_string()),
            recurring: true,
            year: None,
            source: HolydaySource::Default,
        },
        // Flux Holydays (day 50 of each season)
        Holyday {
            key: HolydayKey::SeasonDay {
                season: Season::Chaos,
                day: 50,
            },
            name: "Chaoflux".to_string(),
            description: Some(
                "The midpoint of the Season of Chaos. Celebrate the primordial disorder!".to_string(),
            ),
            greeting: Some("Happy Chaoflux! All Hail Discordia!".to_string()),
            apostle: Some("Hung Mung".to_string()),
            recurring: true,
            year: None,
            source: HolydaySource::Default,
        },
        Holyday {
            key: HolydayKey::SeasonDay {
                season: Season::Discord,
                day: 50,
            },
            name: "Discoflux".to_string(),
            description: Some(
                "The midpoint of the Season of Discord. Embrace the blessed strife!".to_string(),
            ),
            greeting: Some("Happy Discoflux! All Hail Discordia!".to_string()),
            apostle: Some("Dr. Van Van Mojo".to_string()),
            recurring: true,
            year: None,
            source: HolydaySource::Default,
        },
        Holyday {
            key: HolydayKey::SeasonDay {
                season: Season::Confusion,
                day: 50,
            },
            name: "Confuflux".to_string(),
            description: Some(
                "The midpoint of the Season of Confusion. Revel in blessed bewilderment!".to_string(),
            ),
            greeting: Some("Happy Confuflux! All Hail Discordia!".to_string()),
            apostle: Some("Sri Syadasti".to_string()),
            recurring: true,
            year: None,
            source: HolydaySource::Default,
        },
        Holyday {
            key: HolydayKey::SeasonDay {
                season: Season::Bureaucracy,
                day: 50,
            },
            name: "Bureflux".to_string(),
            description: Some(
                "The midpoint of the Season of Bureaucracy. Fill out the forms in triplicate!"
                    .to_string(),
            ),
            greeting: Some("Happy Bureflux! All Hail Discordia!".to_string()),
            apostle: Some("Zarathud".to_string()),
            recurring: true,
            year: None,
            source: HolydaySource::Default,
        },
        Holyday {
            key: HolydayKey::SeasonDay {
                season: Season::Aftermath,
                day: 50,
            },
            name: "Afflux".to_string(),
            description: Some(
                "The midpoint of the Season of Aftermath. Contemplate the chaos behind us!".to_string(),
            ),
            greeting: Some("Happy Afflux! All Hail Discordia!".to_string()),
            apostle: Some("The Elder Malaclypse".to_string()),
            recurring: true,
            year: None,
            source: HolydaySource::Default,
        },
        // St. Tib's Day (intercalary, leap years only)
        Holyday {
            key: HolydayKey::StTibs,
            name: "St. Tib's Day".to_string(),
            description: Some(
                "The intercalary day that occurs in leap years. A day outside the normal calendar, \
                 dedicated to St. Tib, a virgin martyr of no particular importance."
                    .to_string(),
            ),
            greeting: Some(
                "Happy St. Tib's Day! May your paradoxes resolve themselves!".to_string(),
            ),
            apostle: None,
            recurring: true,
            year: None,
            source: HolydaySource::Default,
        },
    ]
}
