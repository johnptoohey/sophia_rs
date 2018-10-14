// this module is transparently re-exported by its parent `graph`

use ::graph::traits::*;
use ::streams::*;
use ::triple::*;

pub struct Inserter<'a, G: ?Sized + 'a> {
    graph: &'a mut G,
    count: usize,
}

impl<'a, G: MutableGraph + ?Sized + 'a> Inserter<'a, G> {
    pub fn new(graph: &'a mut G) -> Self {
        Inserter { graph, count: 0 }
    }
}

impl<'a, G: MutableGraph + ?Sized + 'a> TripleSink for Inserter<'a, G> {
    type Error = G::Error;
    type Outcome = usize;

    fn feed<T: Triple>(&mut self, t: &T) -> Result<(), Self::Error> {
        self.graph.insert(t.s(), t.p(), t.o()).map(|inserted| {
            if inserted {
                self.count += 1;
            }
        })
    }
    fn finish(&mut self) -> Result<Self::Outcome, Self::Error> {
        Ok(self.count)
    }

}


pub struct Remover<'a, G: ?Sized + 'a> {
    graph: &'a mut G,
    count: usize,
}

impl<'a, G: MutableGraph + ?Sized + 'a> Remover<'a, G> {
    pub fn new(graph: &'a mut G) -> Self {
        Remover { graph, count: 0 }
    }
}

impl<'a, G: MutableGraph + ?Sized + 'a> TripleSink for Remover<'a, G> {
    type Error = G::Error;
    type Outcome = usize;

    fn feed<T: Triple>(&mut self, t: &T) -> Result<(), Self::Error> {
        self.graph.remove(t.s(), t.p(), t.o()).map(|removed| {
            if removed {
                self.count += 1;
            }
        })
    }
    fn finish(&mut self) -> Result<Self::Outcome, Self::Error> {
        Ok(self.count)
    }
}