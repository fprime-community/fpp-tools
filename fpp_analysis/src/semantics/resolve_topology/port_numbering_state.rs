use std::collections::BTreeSet;

/// Port numbering state
#[derive(Clone)]
pub struct PortNumberingState {
    /// The used port numbers
    used_port_numbers: BTreeSet<i128>,
    /// The next port number
    next_port_number: i128,
}

impl PortNumberingState {
    /// Marks the specified port number as used and generates
    /// a new one
    pub fn use_port_number(&self, n: i128) -> PortNumberingState {
        let mut s = self.used_port_numbers.clone();
        s.insert(n);
        let n1 = PortNumberingState::get_next_number(self.next_port_number, &s);
        PortNumberingState {
            used_port_numbers: s,
            next_port_number: n1,
        }
    }

    /// Marks the next port number as used and generates a new one
    pub fn use_next_port_number(&self) -> PortNumberingState {
        self.use_port_number(self.next_port_number)
    }

    /// Gets the next port number and updates the state
    pub fn get_port_number(&self) -> (PortNumberingState, i128) {
        let s = self.use_next_port_number();
        (s, self.next_port_number)
    }

    /// Construct an initial state
    pub fn initial(used_port_numbers: BTreeSet<i128>) -> PortNumberingState {
        let next_port_number = PortNumberingState::get_next_number(0, &used_port_numbers);
        PortNumberingState {
            used_port_numbers,
            next_port_number,
        }
    }

    /// Gets the next available port number
    fn get_next_number(from: i128, used: &BTreeSet<i128>) -> i128 {
        let mut n = from;
        while used.contains(&n) {
            n += 1;
        }
        n
    }
}

#[cfg(test)]
mod tests {
    use super::PortNumberingState;
    use std::collections::BTreeSet;

    fn state(used: &[i128]) -> PortNumberingState {
        PortNumberingState::initial(used.iter().copied().collect::<BTreeSet<_>>())
    }

    #[test]
    fn initial_picks_the_lowest_unused_number() {
        assert_eq!(state(&[]).next_port_number, 0);
        assert_eq!(state(&[1, 2]).next_port_number, 0);
        assert_eq!(state(&[0, 1, 3]).next_port_number, 2);
    }

    #[test]
    fn use_port_number_marks_the_number_and_advances_from_the_next_number() {
        let s = state(&[0, 1, 3]).use_port_number(2);
        assert_eq!(s.used_port_numbers, [0, 1, 2, 3].into_iter().collect());
        assert_eq!(s.next_port_number, 4);

        // The search for the next number restarts from the previous next number,
        // not from the number just used, so a number above the gap leaves the
        // next number where it was
        let s = state(&[0, 1, 3]).use_port_number(100);
        assert_eq!(s.used_port_numbers, [0, 1, 3, 100].into_iter().collect());
        assert_eq!(s.next_port_number, 2);
    }

    #[test]
    fn use_next_port_number_consumes_the_next_number() {
        let s = state(&[0, 1, 3]).use_next_port_number();
        assert_eq!(s.used_port_numbers, [0, 1, 2, 3].into_iter().collect());
        assert_eq!(s.next_port_number, 4);

        let s = state(&[]).use_next_port_number().use_next_port_number();
        assert_eq!(s.used_port_numbers, [0, 1].into_iter().collect());
        assert_eq!(s.next_port_number, 2);
    }

    #[test]
    fn get_port_number_returns_the_consumed_number() {
        let (s, n) = state(&[0, 1, 3]).get_port_number();
        assert_eq!(n, 2);
        assert_eq!(s.next_port_number, 4);
    }
}
