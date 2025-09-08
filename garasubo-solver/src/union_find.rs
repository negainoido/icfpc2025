/* =========================
 *   Union-Find
 * ========================= */
#[derive(Clone, Debug)]
struct UF {
    p: Vec<usize>,
    sz: Vec<usize>,
}
impl UF {
    pub fn new() -> Self {
        Self {
            p: Vec::new(),
            sz: Vec::new(),
        }
    }
    pub fn add(&mut self) -> usize {
        let id = self.p.len();
        self.p.push(id);
        self.sz.push(1);
        id
    }
    pub fn find(&mut self, x: usize) -> usize {
        if self.p[x] != x {
            let r = self.find(self.p[x]);
            self.p[x] = r;
        }
        self.p[x]
    }
    pub fn same(&mut self, a: usize, b: usize) -> bool {
        self.find(a) == self.find(b)
    }
    pub fn unite(&mut self, a: usize, b: usize) -> usize {
        let mut a = self.find(a);
        let mut b = self.find(b);
        if a == b {
            return a;
        }
        if self.sz[a] < self.sz[b] {
            std::mem::swap(&mut a, &mut b);
        }
        self.p[b] = a;
        self.sz[a] += self.sz[b];
        a
    }
}
