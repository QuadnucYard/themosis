use kdl::{KdlEntry, KdlNode, KdlValue};
use themosis_core::{SourceId, Span};

use crate::error::StructureError;

/// One value decoded from a KDL entry, together with its exact source span.
pub(crate) struct Decoded<T> {
    value: T,
    span: Span,
}

impl<T> Decoded<T> {
    pub(crate) const fn value(&self) -> &T {
        &self.value
    }

    pub(crate) fn into_parts(self) -> (T, Span) {
        (self.value, self.span)
    }
}

/// Strict scalar conversion supported by the component-style schema decoder.
pub(crate) trait DecodeValue: Sized {
    const EXPECTED: &'static str;

    fn decode(value: &KdlValue) -> Option<Self>;
}

impl DecodeValue for String {
    const EXPECTED: &'static str = "string";

    fn decode(value: &KdlValue) -> Option<Self> {
        value.as_string().map(ToOwned::to_owned)
    }
}

impl DecodeValue for bool {
    const EXPECTED: &'static str = "boolean";

    fn decode(value: &KdlValue) -> Option<Self> {
        value.as_bool()
    }
}

impl DecodeValue for f64 {
    const EXPECTED: &'static str = "number";

    fn decode(value: &KdlValue) -> Option<Self> {
        match value {
            KdlValue::Integer(value) => Some(*value as f64),
            KdlValue::Float(value) => Some(*value),
            KdlValue::String(_) | KdlValue::Bool(_) | KdlValue::Null => None,
        }
    }
}

/// Shared source identity and error accumulator for one KDL document.
pub(crate) struct Decoder {
    source: SourceId,
    errors: Vec<StructureError>,
}

impl Decoder {
    pub(crate) const fn new(source: SourceId) -> Self {
        Self {
            source,
            errors: Vec::new(),
        }
    }

    pub(crate) fn node<'node, 'decoder>(
        &'decoder mut self,
        node: &'node KdlNode,
        context: impl Into<String>,
    ) -> NodeDecoder<'node, 'decoder> {
        NodeDecoder::new(self, node, context.into())
    }

    pub(crate) fn node_span(&self, node: &KdlNode) -> Span {
        let span = node.span();
        self.span(span.offset(), span.len())
    }

    pub(crate) const fn source(&self) -> SourceId {
        self.source
    }

    pub(crate) fn node_name_span(&self, node: &KdlNode) -> Span {
        let span = node.name().span();
        self.span(span.offset(), span.len())
    }

    pub(crate) fn error_at(
        &mut self,
        context: impl Into<String>,
        message: impl Into<String>,
        span: Span,
    ) {
        self.errors.push(StructureError::at(context, message, span));
    }

    pub(crate) fn unexpected_node(&mut self, context: &str, node: &KdlNode) {
        self.error_at(
            context,
            format!("unexpected node '{}'", node.name().value()),
            self.node_name_span(node),
        );
    }

    pub(crate) fn into_errors(mut self) -> Vec<StructureError> {
        self.errors
            .sort_by_key(|error| error.span().map_or(usize::MAX, themosis_core::Span::start));
        self.errors
    }

    fn entry_span(&self, entry: &KdlEntry) -> Span {
        let span = entry.span();
        self.span(span.offset(), span.len())
    }

    fn identifier_span(&self, identifier: &kdl::KdlIdentifier) -> Span {
        let span = identifier.span();
        self.span(span.offset(), span.len())
    }

    fn span(&self, offset: usize, length: usize) -> Span {
        let end = offset
            .checked_add(length)
            .expect("KDL source spans fit within the parsed input");
        Span::new(self.source, offset..end).expect("KDL returns ordered source spans")
    }
}

/// Stateful view of one node that reports every unconsumed schema element.
pub(crate) struct NodeDecoder<'node, 'decoder> {
    decoder: &'decoder mut Decoder,
    node: &'node KdlNode,
    context: String,
    used_entries: Vec<bool>,
    children_claimed: bool,
    finished: bool,
}

impl<'node, 'decoder> NodeDecoder<'node, 'decoder> {
    fn new(decoder: &'decoder mut Decoder, node: &'node KdlNode, context: String) -> Self {
        let mut result = Self {
            decoder,
            node,
            context,
            used_entries: vec![false; node.entries().len()],
            children_claimed: false,
            finished: false,
        };
        result.reject_annotations();
        result
    }

    pub(crate) fn span(&self) -> Span {
        self.decoder.node_span(self.node)
    }

    pub(crate) fn required_argument<T: DecodeValue>(
        &mut self,
        position: usize,
        label: &str,
    ) -> Option<Decoded<T>> {
        let Some(index) = self.argument_entry_index(position) else {
            let span = self.decoder.node_name_span(self.node);
            self.error_at(format!("{label} argument is required"), span);
            return None;
        };

        self.used_entries[index] = true;
        self.decode_entry(index, format!("{label} argument"))
    }

    pub(crate) fn required_property<T: DecodeValue>(&mut self, name: &str) -> Option<Decoded<T>> {
        self.property(name, true)
    }

    pub(crate) fn optional_property<T: DecodeValue>(&mut self, name: &str) -> Option<Decoded<T>> {
        self.property(name, false)
    }

    /// Claims and returns child nodes in their original source order.
    pub(crate) fn children(&mut self) -> &'node [KdlNode] {
        self.children_claimed = true;
        self.node
            .children()
            .map(kdl::KdlDocument::nodes)
            .unwrap_or_default()
    }

    /// Reports arguments, properties, or children that were not claimed by the schema.
    pub(crate) fn finish(mut self) {
        self.report_unclaimed();
    }

    fn report_unclaimed(&mut self) {
        if self.finished {
            return;
        }
        self.finished = true;

        for (index, entry) in self.node.entries().iter().enumerate() {
            if self.used_entries[index] {
                continue;
            }

            let message = entry.name().map_or_else(
                || "unexpected argument".to_owned(),
                |name| format!("unexpected property '{}'", name.value()),
            );
            let span = self.decoder.entry_span(entry);
            self.decoder.error_at(self.context.clone(), message, span);
        }

        if !self.children_claimed
            && let Some(children) = self.node.children()
        {
            for child in children.nodes() {
                self.decoder.unexpected_node(&self.context, child);
            }
        }
    }

    fn reject_annotations(&mut self) {
        if let Some(annotation) = self.node.ty() {
            let span = self.decoder.identifier_span(annotation);
            self.error_at("node type annotations are not supported", span);
        }

        for entry in self.node.entries() {
            if let Some(annotation) = entry.ty() {
                let span = self.decoder.identifier_span(annotation);
                self.error_at("value type annotations are not supported", span);
            }
        }
    }

    fn argument_entry_index(&self, position: usize) -> Option<usize> {
        self.node
            .entries()
            .iter()
            .enumerate()
            .filter(|(_, entry)| entry.name().is_none())
            .nth(position)
            .map(|(index, _)| index)
    }

    fn property<T: DecodeValue>(&mut self, name: &str, required: bool) -> Option<Decoded<T>> {
        let indices = self
            .node
            .entries()
            .iter()
            .enumerate()
            .filter(|(_, entry)| {
                entry
                    .name()
                    .is_some_and(|property| property.value() == name)
            })
            .map(|(index, _)| index)
            .collect::<Vec<_>>();

        let Some((&first, duplicates)) = indices.split_first() else {
            if required {
                let span = self.decoder.node_name_span(self.node);
                self.error_at(format!("property '{name}' is required"), span);
            }
            return None;
        };

        for &index in &indices {
            self.used_entries[index] = true;
        }
        for &duplicate in duplicates {
            let span = self.decoder.entry_span(&self.node.entries()[duplicate]);
            self.error_at(format!("duplicate property '{name}'"), span);
        }

        self.decode_entry(first, format!("property '{name}'"))
    }

    fn decode_entry<T: DecodeValue>(&mut self, index: usize, label: String) -> Option<Decoded<T>> {
        let entry = &self.node.entries()[index];
        let span = self.decoder.entry_span(entry);
        match T::decode(entry.value()) {
            Some(value) => Some(Decoded { value, span }),
            None => {
                self.error_at(
                    format!(
                        "{label} must be a {}, found {}",
                        T::EXPECTED,
                        value_kind(entry.value())
                    ),
                    span,
                );
                None
            }
        }
    }

    fn error_at(&mut self, message: impl Into<String>, span: Span) {
        self.decoder
            .error_at(self.context.clone(), message.into(), span);
    }
}

impl Drop for NodeDecoder<'_, '_> {
    fn drop(&mut self) {
        self.report_unclaimed();
    }
}

fn value_kind(value: &KdlValue) -> &'static str {
    match value {
        KdlValue::String(_) => "string",
        KdlValue::Integer(_) => "integer",
        KdlValue::Float(_) => "floating-point number",
        KdlValue::Bool(_) => "boolean",
        KdlValue::Null => "null",
    }
}

#[cfg(test)]
mod tests {
    use kdl::KdlDocument;
    use themosis_core::SourceId;

    use super::Decoder;

    #[test]
    fn reports_annotations_duplicates_types_and_unclaimed_elements() {
        let input = "(kind)thing (value)\"ok\" 42 extra=1 extra=2 { child }\n";
        let document = KdlDocument::parse_v2(input).expect("input is valid KDL 2");
        let mut decoder = Decoder::new(SourceId::new(3));

        {
            let mut node = decoder.node(&document.nodes()[0], "thing");
            let value = node
                .required_argument::<String>(0, "name")
                .expect("first argument is a string");
            assert_eq!(value.value(), "ok");
            assert!(node.required_property::<String>("extra").is_none());
            node.finish();
        }

        let messages = decoder
            .into_errors()
            .into_iter()
            .map(|error| error.message().to_owned())
            .collect::<Vec<_>>();
        assert_eq!(
            messages,
            [
                "node type annotations are not supported",
                "value type annotations are not supported",
                "unexpected argument",
                "property 'extra' must be a string, found integer",
                "duplicate property 'extra'",
                "unexpected node 'child'",
            ]
        );
    }

    #[test]
    fn type_errors_use_the_exact_entry_span() {
        let input = "thing #true\n";
        let document = KdlDocument::parse_v2(input).expect("input is valid KDL 2");
        let mut decoder = Decoder::new(SourceId::new(9));

        {
            let mut node = decoder.node(&document.nodes()[0], "thing");
            assert!(node.required_argument::<String>(0, "name").is_none());
            node.finish();
        }

        let errors = decoder.into_errors();
        assert_eq!(errors.len(), 1);
        let span = errors[0].span().expect("type error has an entry span");
        assert_eq!(span.source(), SourceId::new(9));
        assert_eq!(span.range(), 6..11);
    }

    #[test]
    fn dropping_a_node_reports_unclaimed_schema_elements() {
        let input = "thing ok extra=1 { child }\n";
        let document = KdlDocument::parse_v2(input).expect("input is valid KDL 2");
        let mut decoder = Decoder::new(SourceId::new(0));

        {
            let mut node = decoder.node(&document.nodes()[0], "thing");
            assert!(node.required_argument::<String>(0, "name").is_some());
        }

        let messages = decoder
            .into_errors()
            .into_iter()
            .map(|error| error.message().to_owned())
            .collect::<Vec<_>>();
        assert_eq!(
            messages,
            ["unexpected property 'extra'", "unexpected node 'child'"]
        );
    }
}
