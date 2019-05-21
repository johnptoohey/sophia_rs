//! Parser for RDF XML.

use std::collections::HashMap;
use std::collections::LinkedList;
use std::fmt::Debug;
use std::io::{BufRead, BufReader, Read};
use std::ops::RangeFrom;
use std::rc::Rc;

use quick_xml::events::BytesEnd;
use quick_xml::events::BytesStart;
use quick_xml::events::BytesText;
use quick_xml::events::Event;

use crate::error::*;
use crate::ns::rdf;
use crate::ns::xml;
use crate::ns::xsd;
use crate::ns::Namespace;
use crate::term::factory::RcTermFactory;
use crate::term::factory::TermFactory;
use crate::term::matcher::TermMatcher;
use crate::term::Term;
use crate::term::TermData;
use crate::triple::Triple;

// ---

#[derive(Clone, Debug, Default)]
pub struct Config;

impl Config {
    #[inline]
    pub fn parse_bufread<'a, B: BufRead + 'a>(
        &self,
        bufread: B,
    ) -> impl Iterator<Item = Result<[Term<Rc<str>>; 3]>> + 'a {
        XmlParser::<_, RcTermFactory>::new(quick_xml::Reader::from_reader(bufread))
    }

    #[inline]
    pub fn parse_read<'a, R: Read + 'a>(
        &self,
        read: R,
    ) -> impl Iterator<Item = Result<[Term<Rc<str>>; 3]>> + 'a {
        self.parse_bufread(BufReader::new(read))
    }

    #[inline]
    pub fn parse_str<'a>(
        &self,
        txt: &'a str,
    ) -> impl Iterator<Item = Result<[Term<Rc<str>>; 3]>> + 'a {
        XmlParser::<_, RcTermFactory>::new(quick_xml::Reader::from_str(txt))
    }
}

// ---

// enum ParsingMode {
//     Node,
//     Predicate,
//     Resource,
// }

// ---

#[derive(Debug, Clone)]
pub struct PrefixMapping<F: TermFactory> {
    default: Option<Namespace<F::TermData>>,
    mapping: HashMap<String, Namespace<F::TermData>>,
    factory: F,
}

impl<F: TermFactory + Default> Default for PrefixMapping<F> {
    fn default() -> Self {
        let mut m = Self {
            default: None,
            mapping: HashMap::new(),
            factory: Default::default(),
        };
        m.add_prefix("xml", "http://www.w3.org/XML/1998/namespace#");
        m
    }
}

impl<F: TermFactory> PrefixMapping<F> {
    pub fn add_prefix(&mut self, prefix: &str, value: &str) {
        if prefix == "_" {
            panic!("reserved prefix")
        } else {
            self.mapping.insert(
                String::from(prefix),
                Namespace::new(self.factory.get_term_data(value)).expect("FIXME"),
            );
        }
    }

    pub fn expand_curie_string(&mut self, curie_str: &str) -> Term<F::TermData> {
        if let Some(separator_idx) = curie_str.chars().position(|c| c == ':') {
            let prefix = &curie_str[..separator_idx];
            let reference = &curie_str[separator_idx + 1..];
            self.expand_curie(&prefix, &reference)
        } else {
            panic!("missing prefix: {}", curie_str)
        }
    }

    pub fn expand_curie(&mut self, prefix: &str, local: &str) -> Term<F::TermData> {
        if let Some(ns) = self.mapping.get(prefix) {
            ns.get(self.factory.get_term_data(local)).expect("FIXME")
        } else {
            panic!("no such namespace")
        }
    }
}

// ---

struct Text<T: TermData> {
    datatype: Option<Term<T>>,
    text: String,
}

impl<T: TermData> Default for Text<T> {
    fn default() -> Self {
        Self {
            datatype: None,
            text: Default::default(),
        }
    }
}

impl<T: TermData> Text<T> {
    fn set_datatype<O: Into<Option<Term<T>>>>(&mut self, datatype: O) {
        self.datatype = datatype.into();
    }

    fn set_text(&mut self, text: String) {
        self.text = text;
    }
}

// ---

struct XmlParser<B: BufRead, F: TermFactory> {
    //
    reader: quick_xml::Reader<B>,
    // The stack of namespaces: should be optimized.
    namespaces: Vec<PrefixMapping<F>>,
    // The stack of `xml:lang`: should be optimized
    lang: Vec<Option<F::TermData>>,
    // The stack of parents (for nested declarations)
    parents: Vec<Term<F::TermData>>,

    // The queue of produced triples
    triples: LinkedList<Result<[Term<F::TermData>; 3]>>,
    // `true` if we are currently in a node element.
    in_node: bool,
    //
    factory: F,
    //
    bnodes: RangeFrom<usize>,
    //
    text: Option<Text<F::TermData>>,
}

impl<B, F> XmlParser<B, F>
where
    B: BufRead,
    F: TermFactory + Clone + Default + Debug,
    <F as TermFactory>::TermData: Debug,
{
    // ---

    // Add a local scope (`lang`, `namespaces`, but not `parents`)
    fn enter_scope(&mut self, e: &BytesStart) {
        // Add a new namespace mapping or copy the last one (OPTIMISE ME)
        let mut ns = self.namespaces.last().unwrap().clone();
        for attr in e.attributes().with_checks(true) {
            let a = attr.expect("FIXME");
            if a.key.starts_with(b"xmlns:") {
                ns.add_prefix(
                    std::str::from_utf8(&a.key[6..]).expect("FIXME"),
                    std::str::from_utf8(&a.value.as_ref()).expect("FIXME"),
                );
            }
        }
        self.namespaces.push(ns);

        // Add current lang to scope or copy last one (OPTIMISE ME)
        let mut lang = self.lang.last().unwrap().clone();
        for attr in e.attributes().with_checks(true) {
            let a = attr.expect("FIXME");
            if a.key == b"xml:lang" {
                lang = Some(
                    self.factory
                        .get_term_data(&a.unescape_and_decode_value(&self.reader).unwrap()),
                );
            }
        }
        self.lang.push(lang);

        // Reset text element
        self.text = None;
    }

    // Exit the local scope
    fn leave_scope(&mut self) {
        self.namespaces.pop();
        self.lang.pop();
        self.text = None;
    }

    // ---

    fn new(reader: quick_xml::Reader<B>) -> Self {
        Self {
            reader,
            parents: Vec::new(),
            namespaces: vec![PrefixMapping::default()],
            triples: LinkedList::new(),
            in_node: false,
            factory: Default::default(),
            bnodes: 0..,
            lang: vec![None],
            text: None,
        }
    }

    // ---

    fn element_start(&mut self, e: &BytesStart) {
        self.enter_scope(e);
        // Ignore top-level rdf:RDF element
        if e.name() != b"rdf:RDF" {
            // Change the current element type
            self.in_node = !self.in_node;
            // Parse as a node of as a property
            if self.in_node {
                self.node_start(e)
            } else {
                self.predicate_start(e)
            }
        }

        println!(
            "Entering {}: {:?}",
            std::str::from_utf8(e.name()).unwrap(),
            self.parents
        );
    }

    fn node_start(&mut self, e: &BytesStart) {
        let ns = self.namespaces.last_mut().unwrap();

        // Separate node subject from other attributes
        let mut properties = HashMap::new();
        let mut subject = None;
        for attr in e.attributes().with_checks(true) {
            let a = attr.expect("FIXME");

            // ignore xmlns attributes (processed in element_start)
            if a.key.starts_with(b"xmlns") {
                continue;
            }

            // try to extract the subject annotation
            let k = ns.expand_curie_string(std::str::from_utf8(a.key).expect("FIXME"));
            let v = a.unescape_and_decode_value(&self.reader).expect("FIXME");
            if k.matches(&rdf::about) {
                if subject.is_none() {
                    subject = Some(self.factory.iri(v).expect("FIXME"));
                } else {
                    panic!("cannot have rdf:ID, rdf:about and rdf:nodeId at the same time")
                }
            } else if k.matches(&rdf::ID) {

            } else if k.matches(&rdf::nodeID) {
                if subject.is_none() {
                    subject = Some(self.factory.bnode(format!("o{}", v)).expect("FIXME"));
                } else {
                    panic!("cannot have rdf:ID, rdf:about and rdf:nodeId at the same time")
                }
            } else if !k.matches(&xml::lang) {
                // Ignore xml:lang attributes
                properties.insert(k, self.factory.literal_dt(v, xsd::string).expect("FIXME"));
            }
        }

        // Get subject and add it to the current nested stack
        let s: Term<_> = subject.unwrap_or(
            self.factory
                .bnode(format!("n{}", self.bnodes.next().unwrap()))
                .expect("FIXME"),
        );
        self.parents.push(s.clone());

        // Add the type as a triple if it is not `rdf:Description`
        let ty = ns.expand_curie_string(std::str::from_utf8(e.name()).expect("FIXME"));
        if ty != rdf::Description {
            self.triples
                .push_back(Ok([s.clone(), self.factory.copy(&rdf::type_), ty]));
        }

        // Add triples described by properties in XML attributes
        for (p, lit) in properties {
            self.triples.push_back(Ok([s.clone(), p, lit]))
        }

        // Add the entity as a triple object if it is not top-level
        if self.parents.len() > 1 {
            let o = s;
            let s = &self.parents[self.parents.len() - 3];
            let p = &self.parents[self.parents.len() - 2];
            self.triples.push_back(Ok([s.clone(), p.clone(), o]));
        }
    }

    fn predicate_start(&mut self, e: &BytesStart) {
        let ns = self.namespaces.last_mut().unwrap();

        // Get the predicate and add it to the current nested stack
        let p = ns.expand_curie_string(std::str::from_utf8(e.name()).expect("FIXME"));
        self.parents.push(p);

        // Get the datatype of the possible literal value, if any
        let mut txt = Text::default();
        for attr in e.attributes().with_checks(true) {
            let a = attr.expect("FIXME");
            if !a.key.starts_with(b"xmlns") {
                let k = ns.expand_curie_string(std::str::from_utf8(a.key).expect("FIXME"));
                if k.matches(&rdf::datatype) {
                    let v = a.unescape_and_decode_value(&self.reader).expect("FIXME");
                    // txt.set_datatype(ns.expand_curie_string(&v));
                    txt.set_datatype(self.factory.iri(v).expect("FIXME"));
                }
            }
        }
        self.text = Some(txt);
    }

    // ---

    fn element_end(&mut self, e: &BytesEnd) {
        println!(
            "Leaving {}: {:?}",
            std::str::from_utf8(e.name()).unwrap(),
            self.parents
        );

        // Change the current element type (if not in rdf:RDF)
        if e.name() != b"rdf:RDF" {
            if !self.in_node {
                self.predicate_end(e);
            }
            self.in_node = !self.in_node;
        }
        self.leave_scope();

        // Remove
        self.parents.pop();
    }

    fn predicate_end(&mut self, e: &BytesEnd) {
        // Build the curie string corresponding
        let ns = self.namespaces.last_mut().unwrap();
        let p = ns.expand_curie_string(std::str::from_utf8(e.name()).expect("FIXME"));

        // Get the literal value
        if let Some(text) = self.text.take() {
            let s = &self.parents[self.parents.len() - 2];
            let o = match (text.datatype, self.lang.last()) {
                (Some(dt), _) => self.factory.literal_dt(text.text, dt).expect("FIXME"),
                (None, Some(Some(l))) => self.factory.literal_lang(text.text, l).expect("FIXME"),
                _ => self
                    .factory
                    .literal_dt(text.text, xsd::string)
                    .expect("FIXME"),
            };
            self.triples.push_back(Ok([s.clone(), p, o]));
        }
    }

    // --- Text elements ----------------------------------------------------

    fn element_text(&mut self, e: &BytesText) {
        if !self.in_node {
            self.predicate_text(e);
        }
    }

    fn predicate_text(&mut self, e: &BytesText) {
        if let Some(text) = &mut self.text {
            text.set_text(e.unescape_and_decode(&self.reader).expect("FIXME"));
        }
    }

    // --- Empty elements ----------------------------------------------------

    fn element_empty(&mut self, e: &BytesStart) {
        self.enter_scope(e);
        if self.in_node {
            self.predicate_empty(e)
        } else {
            self.node_empty(e)
        }
        self.leave_scope();
    }

    fn node_empty(&mut self, e: &BytesStart) {}

    fn predicate_empty(&mut self, e: &BytesStart) {
        let ns = self.namespaces.last_mut().unwrap();
        let p = ns.expand_curie_string(std::str::from_utf8(e.name()).expect("FIXME"));

        let mut object = None;
        for attr in e.attributes().with_checks(true) {
            let a = attr.expect("FIXME");

            // ignore xmlns attributes
            if a.key.starts_with(b"xmlns:") {
                continue;
            }

            // try to extract the annotation object
            let k = ns.expand_curie_string(std::str::from_utf8(a.key).expect("FIXME"));
            let v = a.unescape_and_decode_value(&self.reader).expect("FIXME");
            if k.matches(&rdf::resource) {
                if object.is_none() {
                    object = Some(self.factory.iri(v).expect("FIXME"));
                } else {
                    panic!("cannot have rdf:resource and rdf:nodeId at the same time")
                }
            } else if k.matches(&rdf::nodeID) {
                if object.is_none() {
                    object = Some(self.factory.bnode(format!("o{}", v)).expect("FIXME"));
                } else {
                    panic!("cannot have rdf:resource and rdf:nodeId at the same time")
                }
            }
        }

        let s = self.parents.last().unwrap();
        let o = object.unwrap(); // FIXME
        self.triples.push_back(Ok([s.clone(), p, o]));
    }
}

impl<B, F> Iterator for XmlParser<B, F>
where
    B: BufRead,
    F: TermFactory + Clone + Default + Debug,
    <F as TermFactory>::TermData: Debug,
{
    type Item = Result<[Term<F::TermData>; 3]>;
    fn next(&mut self) -> Option<Self::Item> {
        loop {
            // First make sure to consume the queue.
            if let Some(triple) = self.triples.pop_front() {
                return Some(triple);
            }
            // Then process the next event to maybe produce triples
            match &self.reader.read_event(&mut Vec::new()).unwrap() {
                Event::Eof => return None,
                Event::Start(s) => self.element_start(s),
                Event::Empty(e) => self.element_empty(e),
                Event::End(e) => self.element_end(e),
                Event::Text(t) => self.element_text(t),
                _ => (),
            }
        }
    }
}

// ---

#[cfg(test)]
mod test {

    use std::fmt::Debug;
    use std::fmt::Formatter;
    use std::fmt::Result as FmtResult;

    use crate::graph::inmem::HashGraph;
    use crate::graph::inmem::TermIndexMapU;
    use crate::graph::Graph;
    use crate::ns::dc;
    use crate::ns::xsd;
    use crate::term::factory::RcTermFactory;
    use crate::term::factory::TermFactory;
    use crate::term::IriData;
    use crate::term::StaticTerm;
    use crate::term::Term;
    use crate::triple::stream::TripleSource;
    use crate::triple::Triple;

    pub static GRAMMAR_DESC: &str = "RDF/XML Syntax Specification (Revised)";
    pub static GRAMMAR: StaticTerm = Term::Iri(IriData {
        ns: "http://www.w3.org/TR/rdf-syntax-grammar",
        suffix: None,
        absolute: true,
    });

    type TestGraph = HashGraph<TermIndexMapU<u16, RcTermFactory>>;

    impl Debug for TestGraph {
        fn fmt(&self, f: &mut Formatter) -> FmtResult {
            let mut v = Vec::new();
            for t in self.triples() {
                v.push(t.unwrap());
            }
            v.sort_by_key(|t| (t.s().value(), t.p().value(), t.o().value()));
            v.fmt(f)
        }
    }

    // #[test]
    // fn w3c_test_suite() {
    //     fn do_test_suite() -> io::Result<()> {
    //         let rdf_ext = OsStr::new("rdf");
    //         let nt_ext = OsStr::new("nt");
    //
    //         let suite = Path::new("..").join("rdf-tests").join("rdf-xml");
    //         if !suite.exists() || !suite.is_dir() {
    //             panic!("rdf-tests/rdf-xml not found, can not check W3C test-suite. cf README.md");
    //         }
    //
    //         let mut tested = 0;
    //
    //         for e in read_dir(&suite)? {
    //             let entry = e?;
    //             if entry.file_type()?.is_dir() {
    //                 for c in read_dir(entry.path())? {
    //                     let case = c?;
    //                     if case.path().extension() == Some(rdf_ext) {
    //                         if case.path().with_extension(nt_ext).is_file() {
    //                             println!("{}", case.path().display());
    //
    //                             // the reference N-Triples file
    //                             let ntparser = crate::parser::nt::Config::default();
    //                             let ntfile = File::open(case.path().with_extension(nt_ext))?;
    //                             let mut expected = TestGraph::new();
    //                             ntparser.parse_read(ntfile).in_graph(&mut expected).unwrap();
    //                             // the test XML file
    //                             let xmlparser = super::Config::default();
    //                             let xmlfile = File::open(case.path())?;
    //                             let mut actual = TestGraph::new();
    //                             let res = xmlparser.parse_read(xmlfile).in_graph(&mut actual);
    //
    //                             // check the XML parses without error
    //                             assert!(
    //                                 res.is_ok(),
    //                                 format!("{} should parse without error", case.path().display())
    //                             );
    //                             // check the XML produces the same graph
    //                             pretty_assertions::assert_eq!(
    //                                 actual,
    //                                 expected,
    //                                 "{} does not give expected results",
    //                                 case.path().display()
    //                             );
    //
    //                             tested += 1;
    //                         } else if case.path().to_string_lossy().contains("error") {
    //                             // let xmlparser = super::Config::default();
    //                             // let xmlfile = File::open(case.path())?;
    //                             // let mut actual = TestGraph::new();
    //                             // assert!(
    //                             //     xmlparser.parse_read(xmlfile).in_graph(&mut actual).is_err(),
    //                             //     format!("{} should parse with error", case.path().display())
    //                             // );
    //                             //
    //                             // tested += 1;
    //                         }
    //                     }
    //                 }
    //             }
    //         }
    //
    //         assert_ne!(
    //             tested, 0,
    //             "No test found in W3C test-suite, something must be wrong"
    //         );
    //         Ok(())
    //     }
    //     do_test_suite().unwrap()
    // }

    macro_rules! w3c_example {
        ($name:ident, $xml:literal, $nt:literal) => {
            #[test]
            fn $name() {
                let mut g = TestGraph::new();
                super::Config::default()
                    .parse_str($xml)
                    .in_graph(&mut g)
                    .expect("failed parsing XML file");

                let mut nt = Vec::new();
                for triple in crate::parser::nt::Config::default().parse_str($nt) {
                    nt.push(triple.expect("N-Triples iterator failed"));
                }

                assert_eq!(g.len(), nt.len(), "unexpected number of triples: {:#?}", g);
                for t in nt.into_iter() {
                    assert!(
                        g.contains(t.s(), t.p(), t.o()).expect(".contains failed"),
                        "missing triple: ({:?} {:?} {:?})",
                        t.s(),
                        t.p(),
                        t.o()
                    );
                }
            }
        };
    }

    #[test]
    fn w3c_example_07() {
        let mut f = RcTermFactory::default();
        let mut g = TestGraph::new();
        super::Config::default()
            .parse_str(
                r#"<?xml version="1.0"?>
                    <rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#"
                             xmlns:dc="http://purl.org/dc/elements/1.1/"
                             xmlns:ex="http://example.org/stuff/1.0/">
                      <rdf:Description rdf:about="http://www.w3.org/TR/rdf-syntax-grammar"
                    		   dc:title="RDF/XML Syntax Specification (Revised)">
                        <ex:editor>
                          <rdf:Description ex:fullName="Dave Beckett">
                    	<ex:homePage rdf:resource="http://purl.org/net/dajobe/" />
                          </rdf:Description>
                        </ex:editor>
                      </rdf:Description>
                    </rdf:RDF>
                "#,
            )
            .in_graph(&mut g)
            .expect("failed parsing XML file");

        assert_eq!(g.len(), 4, "unexpected number of triples: {:#?}", g);
        assert!(g
            .contains(
                &GRAMMAR,
                &dc::elements::title,
                &f.literal_dt(GRAMMAR_DESC, xsd::string).unwrap()
            )
            .unwrap());
    }

    // Example 08: 'Complete example of xml:lang'
    w3c_example! {
        w3c_example_08,
        r#"<?xml version="1.0" encoding="utf-8"?>
            <rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#"
                     xmlns:dc="http://purl.org/dc/elements/1.1/">
              <rdf:Description rdf:about="http://www.w3.org/TR/rdf-syntax-grammar">
                <dc:title>RDF/XML Syntax Specification (Revised)</dc:title>
                <dc:title xml:lang="en">RDF/XML Syntax Specification (Revised)</dc:title>
                <dc:title xml:lang="en-US">RDF/XML Syntax Specification (Revised)</dc:title>
              </rdf:Description>

              <rdf:Description rdf:about="http://example.org/buecher/baum" xml:lang="de">
                <dc:title>Der Baum</dc:title>
                <dc:description>Das Buch ist außergewöhnlich</dc:description>
                <dc:title xml:lang="en">The Tree</dc:title>
              </rdf:Description>
            </rdf:RDF>
        "#,
        r#"<http://www.w3.org/TR/rdf-syntax-grammar> <http://purl.org/dc/elements/1.1/title> "RDF/XML Syntax Specification (Revised)" .
           <http://www.w3.org/TR/rdf-syntax-grammar> <http://purl.org/dc/elements/1.1/title> "RDF/XML Syntax Specification (Revised)"@en .
           <http://www.w3.org/TR/rdf-syntax-grammar> <http://purl.org/dc/elements/1.1/title> "RDF/XML Syntax Specification (Revised)"@en-us .
           <http://example.org/buecher/baum> <http://purl.org/dc/elements/1.1/title> "Der Baum"@de .
           <http://example.org/buecher/baum> <http://purl.org/dc/elements/1.1/description> "Das Buch ist au\u00DFergew\u00F6hnlich"@de .
           <http://example.org/buecher/baum> <http://purl.org/dc/elements/1.1/title> "The Tree"@en .
        "#
    }

    // Example 09: 'Complete example of rdf:parseType="Literal"'
    w3c_example! {
        w3c_example_09,
        r#"<?xml version="1.0"?>
            <rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#"
                     xmlns:ex="http://example.org/stuff/1.0/">
              <rdf:Description rdf:about="http://example.org/item01">
                <ex:size rdf:datatype="http://www.w3.org/2001/XMLSchema#int">123</ex:size>
              </rdf:Description>
            </rdf:RDF>
        "#,
        r#"<http://example.org/item01> <http://example.org/stuff/1.0/size> "123"^^<http://www.w3.org/2001/XMLSchema#int> .
        "#
    }

    // Example 11: 'Complete RDF/XML description of graph using rdf:nodeID identifying the blank node'
    w3c_example! {
        w3c_example_11,
        r#"<?xml version="1.0"?>
            <rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#"
                     xmlns:dc="http://purl.org/dc/elements/1.1/"
                     xmlns:ex="http://example.org/stuff/1.0/">
              <rdf:Description rdf:about="http://www.w3.org/TR/rdf-syntax-grammar"
            		   dc:title="RDF/XML Syntax Specification (Revised)">
                <ex:editor rdf:nodeID="abc"/>
              </rdf:Description>

              <rdf:Description rdf:nodeID="abc"
                               ex:fullName="Dave Beckett">
                <ex:homePage rdf:resource="http://purl.org/net/dajobe/"/>
              </rdf:Description>
            </rdf:RDF>
        "#,
        // This is with renamed node IDs
        r#"<http://www.w3.org/TR/rdf-syntax-grammar> <http://purl.org/dc/elements/1.1/title> "RDF/XML Syntax Specification (Revised)" .
           <http://www.w3.org/TR/rdf-syntax-grammar> <http://example.org/stuff/1.0/editor> _:oabc .
           _:oabc <http://example.org/stuff/1.0/fullName> "Dave Beckett" .
           _:oabc <http://example.org/stuff/1.0/homePage> <http://purl.org/net/dajobe/> .
        "#
    }

    // Example 14: 'Complete example with rdf:type'
    w3c_example! {
        w3c_example_14,
        r#"<?xml version="1.0"?>
            <rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#"
                     xmlns:dc="http://purl.org/dc/elements/1.1/"
                     xmlns:ex="http://example.org/stuff/1.0/">
              <rdf:Description rdf:about="http://example.org/thing">
                <rdf:type rdf:resource="http://example.org/stuff/1.0/Document"/>
                <dc:title>A marvelous thing</dc:title>
              </rdf:Description>
            </rdf:RDF>
        "#,
        r#"<http://example.org/thing> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://example.org/stuff/1.0/Document> .
           <http://example.org/thing> <http://purl.org/dc/elements/1.1/title> "A marvelous thing" .
        "#
    }

    // Example 15: 'Complete example using a typed node element to replace an rdf:type'
    w3c_example! {
        w3c_example_15,
        r#"<?xml version="1.0"?>
            <rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#"
                     xmlns:dc="http://purl.org/dc/elements/1.1/"
                     xmlns:ex="http://example.org/stuff/1.0/">
              <ex:Document rdf:about="http://example.org/thing">
                <dc:title>A marvelous thing</dc:title>
              </ex:Document>
            </rdf:RDF>
        "#,
        r#"<http://example.org/thing> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://example.org/stuff/1.0/Document> .
           <http://example.org/thing> <http://purl.org/dc/elements/1.1/title> "A marvelous thing" .
        "#
    }

    // Example 17: 'Complex example using RDF list properties'
    w3c_example! {
        w3c_example_17,
        r#"<?xml version="1.0"?>
            <rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#">
              <rdf:Seq rdf:about="http://example.org/favourite-fruit">
                <rdf:_1 rdf:resource="http://example.org/banana"/>
                <rdf:_2 rdf:resource="http://example.org/apple"/>
                <rdf:_3 rdf:resource="http://example.org/pear"/>
              </rdf:Seq>
            </rdf:RDF>
        "#,
        r#"<http://example.org/favourite-fruit> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://www.w3.org/1999/02/22-rdf-syntax-ns#Seq> .
           <http://example.org/favourite-fruit> <http://www.w3.org/1999/02/22-rdf-syntax-ns#_1> <http://example.org/banana> .
           <http://example.org/favourite-fruit> <http://www.w3.org/1999/02/22-rdf-syntax-ns#_2> <http://example.org/apple> .
           <http://example.org/favourite-fruit> <http://www.w3.org/1999/02/22-rdf-syntax-ns#_3> <http://example.org/pear> .
        "#
    }
}
