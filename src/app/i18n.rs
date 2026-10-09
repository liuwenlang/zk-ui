use gpui_kit::SharedString;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Lang {
    En,
    Zh,
}

impl Lang {
    pub fn toggle(self) -> Self {
        match self {
            Lang::En => Lang::Zh,
            Lang::Zh => Lang::En,
        }
    }

    pub fn tr(self, en: &'static str, zh: &'static str) -> SharedString {
        match self {
            Lang::En => en.into(),
            Lang::Zh => zh.into(),
        }
    }
}
