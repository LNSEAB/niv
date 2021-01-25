use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum Order {
    Name,
    UpdatedDate,
    FileSize,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum Comparison {
    Ascending,
    Descending,
}

#[derive(Debug)]
pub struct Directory {
    paths: Vec<PathBuf>,
    index: isize,
    order: Order,
    comp: Comparison,
    lookahead: isize,
}

impl Directory {
    pub fn new<T, U>(
        dir: T,
        exts: &Vec<String>,
        order: Order,
        comp: Comparison,
        lookahead: isize,
        init: Option<U>,
    ) -> Self
    where
        T: AsRef<Path>,
        U: AsRef<Path>,
    {
        assert!(dir.as_ref().is_dir());
        let paths = dir
            .as_ref()
            .read_dir()
            .unwrap()
            .filter_map(|entry| {
                let path = entry.ok()?.path();
                if !path.is_file() {
                    return None;
                }
                let path_ext = path.extension()?;
                exts.iter()
                    .find(|ext| path_ext == ext.as_str())
                    .map(|_| path)
            })
            .collect::<Vec<_>>();
        let index = init.map_or(0, |i| {
            paths.iter().position(|p| p == i.as_ref()).unwrap_or(0)
        }) as isize;
        let mut obj = Self {
            paths,
            index,
            order,
            lookahead,
            comp,
        };
        obj.change_order(order, comp);
        obj
    }

    pub fn index(&self) -> usize {
        self.index as usize
    }

    pub fn len(&self) -> usize {
        self.paths.len()
    }

    pub fn current(&self) -> Option<&Path> {
        if self.paths.is_empty() {
            None
        } else {
            Some(self.paths[self.index as usize].as_ref())
        }
    }

    pub fn next(&mut self) -> Vec<PathBuf> {
        if self.paths.is_empty() {
            return vec![];
        }
        if self.index < self.paths.len() as isize - 1 {
            self.index += 1;
            let n = self.index + self.lookahead;
            let n = if n >= self.paths.len() as isize {
                self.paths.len()
            } else {
                n as usize
            };
            self.paths[self.index as usize..n]
                .iter()
                .cloned()
                .collect::<Vec<_>>()
        } else {
            vec![self.paths[self.paths.len() - 1].clone()]
        }
    }

    pub fn prev(&mut self) -> Vec<PathBuf> {
        if self.paths.is_empty() {
            return vec![];
        }
        if self.index > 0 {
            self.index -= 1;
            let n = self.index - self.lookahead;
            let n = if n < 0 { 0 } else { n as usize };
            self.paths[n..self.index as usize + 1]
                .iter()
                .cloned()
                .collect::<Vec<_>>()
        } else {
            vec![self.paths[0].clone()]
        }
    }

    pub fn change_order(&mut self, order: Order, comp: Comparison) {
        self.order = order;
        self.comp = comp;
        if self.paths.is_empty() {
            return;
        }
        let current = self.paths[self.index as usize].clone();
        match self.order {
            Order::Name => match self.comp {
                Comparison::Ascending => self.paths.sort_by(|a, b| a.cmp(b)),
                Comparison::Descending => self.paths.sort_by(|a, b| b.cmp(a)),
            },
            Order::UpdatedDate => {
                let f = |a: &Path, b: &Path| -> std::cmp::Ordering {
                    let a = a
                        .metadata()
                        .ok()
                        .and_then(|meta| meta.modified().ok())
                        .and_then(|modified| modified.duration_since(SystemTime::UNIX_EPOCH).ok())
                        .map(|d| d.as_secs())
                        .unwrap_or(std::u64::MAX);
                    let b = b
                        .metadata()
                        .ok()
                        .and_then(|meta| meta.modified().ok())
                        .and_then(|modified| modified.duration_since(SystemTime::UNIX_EPOCH).ok())
                        .map(|d| d.as_secs())
                        .unwrap_or(std::u64::MAX);
                    a.cmp(&b)
                };
                match self.comp {
                    Comparison::Ascending => self.paths.sort_by(|a, b| f(a, b)),
                    Comparison::Descending => self.paths.sort_by(|a, b| f(b, a)),
                }
            }
            Order::FileSize => {
                let f = |a: &Path, b: &Path| -> std::cmp::Ordering {
                    let a = a.metadata().map(|meta| meta.len()).unwrap_or(std::u64::MAX);
                    let b = b.metadata().map(|meta| meta.len()).unwrap_or(std::u64::MAX);
                    a.cmp(&b)
                };
                match self.comp {
                    Comparison::Ascending => self.paths.sort_by(|a, b| f(a, b)),
                    Comparison::Descending => self.paths.sort_by(|a, b| f(b, a)),
                }
            }
        }
        self.paths = self
            .paths
            .iter()
            .filter(|path| path.is_file())
            .cloned()
            .collect::<Vec<_>>();
        self.index = self.paths.iter().position(|p| *p == current).unwrap_or(0) as isize;
    }
}
