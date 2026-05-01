//! A minimal row-major tensor used to pass data between layers.

use crate::fixed_point::FixedPoint;

/// A 2D tensor stored in row-major order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tensor {
    data: Vec<FixedPoint>,
    rows: usize,
    cols: usize,
}

impl Tensor {
    /// Build a tensor from a flat vector, validating the shape.
    pub fn new(data: Vec<FixedPoint>, rows: usize, cols: usize) -> Option<Self> {
        if data.len() == rows * cols {
            Some(Self { data, rows, cols })
        } else {
            None
        }
    }

    /// Number of rows.
    pub fn rows(&self) -> usize {
        self.rows
    }

    /// Number of columns.
    pub fn cols(&self) -> usize {
        self.cols
    }

    /// Get the element at `(r, c)`, if in bounds.
    pub fn get(&self, r: usize, c: usize) -> Option<FixedPoint> {
        if r < self.rows && c < self.cols {
            Some(self.data[r * self.cols + c])
        } else {
            None
        }
    }

    /// Reshape in place without copying, if the element count matches.
    pub fn reshape(&mut self, rows: usize, cols: usize) -> bool {
        if rows * cols == self.data.len() {
            self.rows = rows;
            self.cols = cols;
            true
        } else {
            false
        }
    }
}
