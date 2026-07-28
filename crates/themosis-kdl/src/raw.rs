use knus::{ast::Literal, span::Span};

#[derive(knus::Decode)]
#[knus(span_type = Span)]
pub(crate) enum RawRoot {
    Theme(RawTheme),
}

#[derive(knus::Decode)]
#[knus(span_type = Span)]
pub(crate) struct RawTheme {
    #[knus(argument)]
    pub(crate) name: String,
    #[knus(children)]
    pub(crate) children: Vec<RawThemeChild>,
}

#[derive(knus::Decode)]
#[knus(span_type = Span)]
pub(crate) enum RawThemeChild {
    Tokens(RawSourcePath),
    Import(RawSourcePath),
    Style(RawStyle),
}

#[derive(knus::Decode)]
#[knus(span_type = Span)]
pub(crate) struct RawSourcePath {
    #[knus(argument)]
    pub(crate) path: String,
}

#[derive(knus::Decode)]
#[knus(span_type = Span)]
pub(crate) struct RawStyle {
    #[knus(argument)]
    pub(crate) name: String,
    #[knus(property)]
    pub(crate) target: String,
    #[knus(property)]
    pub(crate) extends: Option<String>,
    #[knus(children)]
    pub(crate) children: Vec<RawStyleChild>,
}

#[derive(knus::Decode)]
#[knus(span_type = Span)]
pub(crate) enum RawStyleChild {
    Boolean(RawBooleanProperty),
    Number(RawNumberProperty),
    String(RawStringProperty),
    Token(RawStringProperty),
    Resource(RawStringProperty),
    State(RawState),
}

#[derive(knus::Decode)]
#[knus(span_type = Span)]
pub(crate) struct RawState {
    #[knus(argument)]
    pub(crate) name: String,
    #[knus(children)]
    pub(crate) properties: Vec<RawProperty>,
}

#[derive(knus::Decode)]
#[knus(span_type = Span)]
pub(crate) enum RawProperty {
    Boolean(RawBooleanProperty),
    Number(RawNumberProperty),
    String(RawStringProperty),
    Token(RawStringProperty),
    Resource(RawStringProperty),
}

#[derive(knus::Decode)]
#[knus(span_type = Span)]
pub(crate) struct RawBooleanProperty {
    #[knus(argument)]
    pub(crate) name: String,
    #[knus(argument)]
    pub(crate) value: bool,
}

#[derive(knus::Decode)]
#[knus(span_type = Span)]
pub(crate) struct RawNumberProperty {
    #[knus(argument)]
    pub(crate) name: String,
    #[knus(argument)]
    pub(crate) value: Literal,
}

#[derive(knus::Decode)]
#[knus(span_type = Span)]
pub(crate) struct RawStringProperty {
    #[knus(argument)]
    pub(crate) name: String,
    #[knus(argument)]
    pub(crate) value: String,
}
