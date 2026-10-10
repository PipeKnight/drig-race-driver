//! Bounded sparse equation lifecycle used by the selected vehicle force solver.
//!
//! Vehicle row production, active limits and event advancement remain separate.
//! Selection requires closed groups: every row term must name a selected variable.
use crate::Error;

pub const CAPACITY: usize = 40;
pub const MAX_TERMS: usize = 8;
pub const SNAPSHOT_WORDS: usize = 884;

#[derive(Clone, Copy, Debug, Default)]
struct Variable {
    value: f32,
    fixed: bool,
    solved: bool,
}

#[derive(Clone, Debug, Default)]
struct Row {
    count: usize,
    coefficients: [f32; MAX_TERMS],
    variables: [usize; MAX_TERMS],
    rhs: f32,
    selected: bool,
}

#[derive(Clone, Debug)]
pub struct System {
    variables: Vec<Variable>,
    rows: Vec<Row>,
}

impl System {
    pub fn new(variables: usize) -> Result<Self, Error> {
        if variables > CAPACITY {
            return Err(Error("too many force variables"));
        }
        Ok(Self {
            variables: vec![Variable::default(); variables],
            rows: Vec::new(),
        })
    }

    /// Append in input order; fixed terms reduce RHS even if their coefficient is zero.
    pub fn add(&mut self, terms: &[(usize, f32)], rhs: f32) -> Result<(), Error> {
        if self.rows.len() == CAPACITY
            || terms.len() > MAX_TERMS
            || !rhs.is_finite()
            || terms
                .iter()
                .any(|(id, c)| *id >= self.variables.len() || !c.is_finite())
        {
            return Err(Error("invalid force equation"));
        }
        let mut row = Row {
            rhs,
            ..Row::default()
        };
        for &(id, coefficient) in terms {
            let variable = self.variables[id];
            if variable.fixed {
                row.rhs -= variable.value * coefficient;
            } else if coefficient != 0. {
                row.coefficients[row.count] = coefficient;
                row.variables[row.count] = id;
                row.count += 1;
            }
        }
        self.rows.push(row);
        Ok(())
    }

    /// Substitute the first occurrence in each row; swap-last removal retains tail storage.
    pub fn fix(&mut self, id: usize, value: f32) -> Result<(), Error> {
        if id >= self.variables.len() || !value.is_finite() {
            return Err(Error("invalid fixed force variable"));
        }
        for row in &mut self.rows {
            if let Some(index) = row.variables[..row.count].iter().position(|v| *v == id) {
                row.rhs -= row.coefficients[index] * value;
                row.coefficients[index] = row.coefficients[row.count - 1];
                row.variables[index] = row.variables[row.count - 1];
                row.count -= 1;
            }
        }
        self.variables[id].value = value;
        self.variables[id].fixed = true;
        Ok(())
    }

    /// Solve a nonempty square closed group. Failure still marks its selected rows.
    pub fn select(&mut self, rows: &[usize], variables: &[usize]) -> Result<bool, Error> {
        self.validate_selection(rows, variables)?;
        let n = variables.len();
        let mut matrix = vec![0.; n * n];
        let mut rhs = Vec::with_capacity(n);
        for (i, &id) in rows.iter().enumerate() {
            let row = &mut self.rows[id];
            for j in 0..row.count {
                let column = variables
                    .iter()
                    .position(|v| *v == row.variables[j])
                    .expect("closed group");
                // The original assigns, rather than sums, duplicate variable terms.
                matrix[i * n + column] = row.coefficients[j];
            }
            rhs.push(row.rhs);
            row.selected = true;
        }
        let Some(values) = eliminate(&mut matrix, &mut rhs) else {
            return Ok(false);
        };
        for (&id, value) in variables.iter().zip(values) {
            self.variables[id].value = value;
            self.variables[id].solved = true;
        }
        Ok(true)
    }

    fn validate_selection(&self, rows: &[usize], variables: &[usize]) -> Result<(), Error> {
        if variables.is_empty()
            || variables.len() > CAPACITY
            || rows.len() != variables.len()
            || rows.iter().any(|id| *id >= self.rows.len())
            || variables.iter().any(|id| *id >= self.variables.len())
            || variables
                .iter()
                .enumerate()
                .any(|(i, id)| variables[..i].contains(id))
            || rows
                .iter()
                .enumerate()
                .any(|(i, id)| rows[..i].contains(id))
            || rows.iter().any(|id| {
                self.rows[*id].variables[..self.rows[*id].count]
                    .iter()
                    .any(|id| !variables.contains(id))
            })
        {
            return Err(Error("invalid or unclosed force selection"));
        }
        Ok(())
    }

    /// Select the first N remaining rows for N unknowns, then clear temporary marks.
    /// An unsafe underdetermined/unclosed boundary is rejected before mutation.
    pub fn remaining(&mut self) -> Result<bool, Error> {
        let variables: Vec<_> = self
            .variables
            .iter()
            .enumerate()
            .filter_map(|(i, v)| (!v.fixed && !v.solved).then_some(i))
            .collect();
        let rows: Vec<_> = self
            .rows
            .iter()
            .enumerate()
            .filter_map(|(i, r)| (!r.selected && r.count != 0).then_some(i))
            .take(variables.len())
            .collect();
        let success = if variables.is_empty() {
            true
        } else {
            self.select(&rows, &variables)?
        };
        for row in &mut self.rows {
            row.selected = false;
        }
        for variable in &mut self.variables {
            variable.solved = false;
        }
        Ok(success)
    }

    pub fn value(&self, id: usize) -> Option<f32> {
        self.variables.get(id).map(|v| v.value)
    }

    /// Canonical bounded reference layout, including inactive slots and stale row tails.
    pub fn snapshot(&self) -> [u32; SNAPSHOT_WORDS] {
        let mut out = [0; SNAPSHOT_WORDS];
        out[0] = self.rows.len() as u32;
        out[2] = self.variables.len() as u32;
        out[3] = self.rows.len() as u32;
        for (i, variable) in self.variables.iter().enumerate() {
            out[4 + i * 2] = variable.value.to_bits();
            out[5 + i * 2] = u32::from(variable.fixed) | (u32::from(variable.solved) << 8);
        }
        for (i, row) in self.rows.iter().enumerate() {
            let offset = 84 + i * 19;
            out[offset] = row.count as u32;
            for j in 0..MAX_TERMS {
                out[offset + 1 + j] = row.coefficients[j].to_bits();
                out[offset + 9 + j] = row.variables[j] as u32;
            }
            out[offset + 17] = row.rhs.to_bits();
            out[offset + 18] = u32::from(row.selected);
            out[844 + i] = i as u32;
        }
        out
    }
}

#[allow(clippy::assign_op_pattern)] // Preserve the observed SSE operand order.
fn eliminate(matrix: &mut [f32], rhs: &mut [f32]) -> Option<Vec<f32>> {
    let n = rhs.len();
    let mut norms: Vec<_> = matrix
        .chunks(n)
        .map(|row| row.iter().fold(0., |sum, v| v * v + sum))
        .collect();
    for column in 0..n - 1 {
        let mut pivot = column;
        for row in column + 1..n {
            let best = matrix[pivot * n + column];
            let candidate = matrix[row * n + column];
            if best * (best / norms[pivot]) < (candidate / norms[row]) * candidate {
                pivot = row;
            }
        }
        if pivot != column {
            for j in 0..n {
                matrix.swap(column * n + j, pivot * n + j);
            }
            rhs.swap(column, pivot);
            norms.swap(column, pivot);
        }
        if matrix[column * n + column] == 0. {
            return None;
        }
        for row in column + 1..n {
            let factor = (-1. / matrix[column * n + column]) * matrix[row * n + column];
            let mut norm = 0.;
            for j in column..n {
                let value = matrix[column * n + j] * factor + matrix[row * n + j];
                matrix[row * n + j] = value;
                norm = value * value + norm;
            }
            norms[row] = norm;
            rhs[row] = factor * rhs[column] + rhs[row];
        }
    }
    let mut values = vec![0.; n];
    for row in (0..n).rev() {
        let mut value = rhs[row];
        for j in row + 1..n {
            value -= values[j] * matrix[row * n + j];
        }
        // Deliberately retain the original unchecked last pivot division.
        values[row] = value / matrix[row * n + row];
    }
    Some(values)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixes_then_solves_and_clears_marks() {
        let mut system = System::new(3).unwrap();
        system.add(&[(0, 2.), (1, 3.), (2, 1.)], 12.).unwrap();
        system.add(&[(1, 2.), (2, -1.)], 3.).unwrap();
        system.fix(0, 2.).unwrap();
        assert!(system.remaining().unwrap());
        assert_eq!(system.value(1), Some(2.2));
        assert_eq!(system.value(2), Some(1.4));
        assert_eq!(system.snapshot()[5], 1);
        assert_eq!(system.snapshot()[7], 0);
        assert_eq!(system.snapshot()[84 + 18], 0);
    }

    #[test]
    fn rejects_unclosed_group_before_marking() {
        let mut system = System::new(2).unwrap();
        system.add(&[(0, 1.), (1, 2.)], 3.).unwrap();
        let before = system.snapshot();
        assert!(system.select(&[0], &[0]).is_err());
        assert_eq!(system.snapshot(), before);
    }

    #[test]
    fn zero_elimination_pivot_fails_but_last_pivot_is_unchecked() {
        let mut system = System::new(2).unwrap();
        system.add(&[(1, 1.)], 2.).unwrap();
        system.add(&[(1, 2.)], 3.).unwrap();
        assert!(!system.remaining().unwrap());
        assert_eq!(system.value(1), Some(0.));
        let mut system = System::new(1).unwrap();
        system.add(&[], 1.).unwrap();
        assert!(system.select(&[0], &[0]).unwrap());
        assert_eq!(system.value(0), Some(f32::INFINITY));
    }
}
