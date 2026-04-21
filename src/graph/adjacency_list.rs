use super::{IterScan, MarkedIterScan};
use std::{marker::PhantomData, ops::Range};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct Adjacency {
    pub id: usize,
    pub to: usize,
}
impl Adjacency {
    pub fn new(id: usize, to: usize) -> Adjacency {
        Adjacency { id, to }
    }
}
#[derive(Clone, Debug, Default)]
pub struct AdjacencyListGraph {
    pub vsize: usize,
    pub esize: usize,
    pub graph: Vec<Vec<Adjacency>>,
}
impl AdjacencyListGraph {
    pub fn new(vsize: usize) -> AdjacencyListGraph {
        AdjacencyListGraph {
            vsize,
            esize: 0,
            graph: vec![vec![]; vsize],
        }
    }
    pub fn add_edge(&mut self, from: usize, to: usize) {
        self.graph[from].push(Adjacency::new(self.esize, to));
        self.esize += 1;
    }
    pub fn add_undirected_edge(&mut self, u: usize, v: usize) {
        self.graph[u].push(Adjacency::new(self.esize, v));
        self.graph[v].push(Adjacency::new(self.esize, u));
        self.esize += 1;
    }
    pub fn vertices(&self) -> Range<usize> {
        0..self.vsize
    }
    pub fn adjacency(&self, from: usize) -> &Vec<Adjacency> {
        &self.graph[from]
    }
}

pub struct AdjacencyListGraphScanner<U, T>
where
    for<'a> U: IterScan<Output<'a> = usize>,
    T: IterScan,
{
    vsize: usize,
    esize: usize,
    directed: bool,
    _marker: PhantomData<fn() -> (U, T)>,
}

impl<U, T> AdjacencyListGraphScanner<U, T>
where
    for<'a> U: IterScan<Output<'a> = usize>,
    T: IterScan,
{
    pub fn new(vsize: usize, esize: usize, directed: bool) -> Self {
        Self {
            vsize,
            esize,
            directed,
            _marker: PhantomData,
        }
    }
}

impl<U, T> MarkedIterScan for AdjacencyListGraphScanner<U, T>
where
    for<'a> U: IterScan<Output<'a> = usize>,
    T: IterScan,
{
    type Output<'a> = (AdjacencyListGraph, Vec<<T as IterScan>::Output<'a>>);
    fn mscan<'a, I: Iterator<Item = &'a str>>(self, iter: &mut I) -> Option<Self::Output<'a>> {
        let mut graph = AdjacencyListGraph::new(self.vsize);
        let mut rest = Vec::with_capacity(self.esize);
        for _ in 0..self.esize {
            let (from, to) = (U::scan(iter)?, U::scan(iter)?);
            if self.directed {
                graph.add_edge(from, to);
            } else {
                graph.add_undirected_edge(from, to);
            }
            rest.push(T::scan(iter)?);
        }
        Some((graph, rest))
    }
}
