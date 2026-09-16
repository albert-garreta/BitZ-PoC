//! Borrowed table views. Native adapters assemble split words only when read.
pub(crate) trait OuterRows: Sync {
    type AB: Copy + Send + Sync;
    type C: Copy + Send + Sync;
    fn dimensions(&self) -> (usize, usize, usize);
    fn a(&self, row: usize) -> Self::AB;
    fn b(&self, row: usize) -> Self::AB;
    fn c(&self, row: usize) -> Self::C;
}

pub(super) struct SliceRows<'a, AB, C> {
    pub ax: &'a [AB],
    pub bx: &'a [AB],
    pub cx: &'a [C],
}
impl<AB: Copy + Send + Sync, C: Copy + Send + Sync> OuterRows for SliceRows<'_, AB, C> {
    type AB = AB;
    type C = C;
    fn dimensions(&self) -> (usize, usize, usize) {
        (self.ax.len(), self.bx.len(), self.cx.len())
    }
    #[inline(always)]
    fn a(&self, row: usize) -> AB {
        self.ax[row]
    }
    #[inline(always)]
    fn b(&self, row: usize) -> AB {
        self.bx[row]
    }
    #[inline(always)]
    fn c(&self, row: usize) -> C {
        self.cx[row]
    }
}
