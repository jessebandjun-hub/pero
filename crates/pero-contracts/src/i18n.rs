use std::{collections::BTreeMap, sync::OnceLock};

/// Language tag stored in the Host settings file.
///
/// [`normalize`] maps system tags onto a catalog. A stored preference accepts
/// only the two catalog tags, so an unrecognized value is not rewritten.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LocalePreference {
    En,
    ZhCn,
}

impl LocalePreference {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::En => "en",
            Self::ZhCn => "zh-CN",
        }
    }

    pub fn parse(tag: &str) -> Option<Self> {
        match tag {
            "en" => Some(Self::En),
            "zh-CN" => Some(Self::ZhCn),
            _ => None,
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum LocaleSettingsRequest {
    Get,
    Set { locale: String },
}

impl LocaleSettingsRequest {
    /// `settings.locale.get` has only `method`. `settings.locale` has `method`
    /// and `locale`. Any other field is rejected here; the tag itself is
    /// checked by [`LocalePreference::parse`] when the Host stores it.
    pub fn parse(value: &serde_json::Value) -> Option<Self> {
        let object = value.as_object()?;
        match (object.get("method")?.as_str()?, object.len()) {
            ("settings.locale.get", 1) => Some(Self::Get),
            ("settings.locale", 2) => Some(Self::Set {
                locale: object.get("locale")?.as_str()?.to_string(),
            }),
            _ => None,
        }
    }
}

pub fn normalize(tag: &str) -> &'static str {
    let tag = tag.replace('_', "-").to_ascii_lowercase();
    if tag == "zh" || tag == "zh-cn" || tag.starts_with("zh-hans") {
        "zh-CN"
    } else {
        "en"
    }
}

fn catalog(locale: &str) -> &'static BTreeMap<String, String> {
    static EN: OnceLock<BTreeMap<String, String>> = OnceLock::new();
    static ZH: OnceLock<BTreeMap<String, String>> = OnceLock::new();
    if normalize(locale) == "zh-CN" {
        ZH.get_or_init(|| {
            serde_json::from_str(include_str!("../../../locales/zh-CN.json"))
                .expect("Chinese catalog")
        })
    } else {
        EN.get_or_init(|| {
            serde_json::from_str(include_str!("../../../locales/en.json")).expect("English catalog")
        })
    }
}

pub fn text(locale: &str, key: &str) -> &'static str {
    catalog(locale)
        .get(key)
        .or_else(|| catalog("en").get(key))
        .map(String::as_str)
        .unwrap_or("[missing translation]")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn locales_and_catalogs_are_consistent() {
        assert_eq!(normalize("zh_Hans_CN"), "zh-CN");
        assert_eq!(normalize("en-US"), "en");
        assert_eq!(normalize("fr"), "en");
        assert_eq!(LocalePreference::parse("en"), Some(LocalePreference::En));
        assert_eq!(
            LocalePreference::parse("zh-CN").map(LocalePreference::as_str),
            Some("zh-CN")
        );
        for tag in ["fr", "zh-cn", "zh_CN", "en-US", ""] {
            assert_eq!(LocalePreference::parse(tag), None, "{tag}");
        }
        assert_eq!(
            LocaleSettingsRequest::parse(&serde_json::json!({"method":"settings.locale.get"})),
            Some(LocaleSettingsRequest::Get)
        );
        assert_eq!(
            LocaleSettingsRequest::parse(
                &serde_json::json!({"method":"settings.locale","locale":"zh-CN"})
            ),
            Some(LocaleSettingsRequest::Set {
                locale: "zh-CN".into()
            })
        );
        let rejected = LocaleSettingsRequest::parse(
            &serde_json::json!({"method":"settings.locale","locale":"fr"}),
        );
        assert_eq!(
            rejected,
            Some(LocaleSettingsRequest::Set {
                locale: "fr".into()
            })
        );
        for value in [
            serde_json::json!({"method":"settings.locale.get","extra":true}),
            serde_json::json!({"method":"settings.locale","locale":"en","extra":true}),
            serde_json::json!({"method":"settings.locale"}),
            serde_json::json!({"method":"settings.locale","locale":1}),
        ] {
            assert_eq!(LocaleSettingsRequest::parse(&value), None, "{value}");
        }
        assert_eq!(text("fr", "nav.tasks"), "Tasks");
        assert_eq!(
            catalog("en").keys().collect::<Vec<_>>(),
            catalog("zh-CN").keys().collect::<Vec<_>>()
        );
        for key in catalog("en").keys() {
            assert!(!text("zh-CN", key).is_empty());
            assert_eq!(
                text("en", key).contains("{count}"),
                text("zh-CN", key).contains("{count}")
            );
        }
    }
}
