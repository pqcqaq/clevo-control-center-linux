use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LanguagePreference {
    #[default]
    System,
    SimplifiedChinese,
    English,
}

impl LanguagePreference {
    pub const ALL: [Self; 3] = [Self::System, Self::SimplifiedChinese, Self::English];

    pub fn resolved(self) -> UiLanguage {
        match self {
            Self::System => detect_system_language(),
            Self::SimplifiedChinese => UiLanguage::SimplifiedChinese,
            Self::English => UiLanguage::English,
        }
    }

    pub fn label(self, language: UiLanguage) -> &'static str {
        match self {
            Self::System => language.pick("跟随系统", "System default"),
            Self::SimplifiedChinese => "简体中文",
            Self::English => "English",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UiLanguage {
    SimplifiedChinese,
    English,
}

impl UiLanguage {
    pub fn pick<'a>(self, simplified_chinese: &'a str, english: &'a str) -> &'a str {
        match self {
            Self::SimplifiedChinese => simplified_chinese,
            Self::English => english,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ThemeColor {
    #[default]
    Amber,
    Cyan,
    Emerald,
    Rose,
}

impl ThemeColor {
    pub const ALL: [Self; 4] = [Self::Amber, Self::Cyan, Self::Emerald, Self::Rose];

    pub fn label(self, language: UiLanguage) -> &'static str {
        match self {
            Self::Amber => language.pick("琥珀", "Amber"),
            Self::Cyan => language.pick("青色", "Cyan"),
            Self::Emerald => language.pick("翠绿", "Emerald"),
            Self::Rose => language.pick("玫红", "Rose"),
        }
    }
}

fn detect_system_language() -> UiLanguage {
    ["LANGUAGE", "LC_ALL", "LC_MESSAGES", "LANG"]
        .into_iter()
        .find_map(|name| std::env::var(name).ok().filter(|value| !value.is_empty()))
        .map_or(UiLanguage::English, |locale| language_from_locale(&locale))
}

fn language_from_locale(locale: &str) -> UiLanguage {
    let primary = locale
        .split(':')
        .next()
        .unwrap_or(locale)
        .trim()
        .to_ascii_lowercase();
    if primary == "zh" || primary.starts_with("zh_") || primary.starts_with("zh-") {
        UiLanguage::SimplifiedChinese
    } else {
        UiLanguage::English
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chinese_locale_variants_resolve_to_chinese() {
        assert_eq!(
            language_from_locale("zh_CN.UTF-8"),
            UiLanguage::SimplifiedChinese
        );
        assert_eq!(
            language_from_locale("zh-Hans"),
            UiLanguage::SimplifiedChinese
        );
        assert_eq!(
            language_from_locale("zh_CN:en_US"),
            UiLanguage::SimplifiedChinese
        );
    }

    #[test]
    fn non_chinese_locale_resolves_to_english() {
        assert_eq!(language_from_locale("en_US.UTF-8"), UiLanguage::English);
        assert_eq!(language_from_locale("C.UTF-8"), UiLanguage::English);
    }

    #[test]
    fn preference_defaults_preserve_existing_product_style() {
        assert_eq!(LanguagePreference::default(), LanguagePreference::System);
        assert_eq!(ThemeColor::default(), ThemeColor::Amber);
    }
}
