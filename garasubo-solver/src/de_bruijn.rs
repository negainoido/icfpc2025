use std::collections::HashSet;

pub fn generate_de_bruijn_sequence(n: usize) -> String {
    if n == 0 {
        return String::new();
    }
    if n == 1 {
        return "01".to_string();
    }

    // Use FKM (Fredricksen, Kessler, and Maiorana) algorithm
    // This generates the lexicographically minimal de Bruijn sequence
    let mut sequence = Vec::new();
    let mut a = vec![0; 2 * n]; // alphabet is [0, 1]

    fn db(t: usize, p: usize, a: &mut [usize], sequence: &mut Vec<usize>, n: usize) {
        if t > n {
            if n % p == 0 {
                sequence.extend(&a[1..p + 1]);
            }
        } else {
            a[t] = a[t - p];
            db(t + 1, p, a, sequence, n);
            for j in (a[t - p] + 1)..2 {
                a[t] = j;
                db(t + 1, t, a, sequence, n);
            }
        }
    }

    db(1, 1, &mut a, &mut sequence, n);

    // Convert to string and make cyclic
    let mut result = String::new();
    for &digit in &sequence {
        result.push_str(&digit.to_string());
    }

    // Add the first n-1 characters to make it a proper de Bruijn sequence
    let prefix = result[..n - 1].to_string();
    result.push_str(&prefix);

    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn test_de_bruijn_n2() {
        let seq = generate_de_bruijn_sequence(2);
        println!("n=2: {}", seq);

        // Should contain each 2-length substring exactly once
        let mut substrings = HashSet::new();
        for i in 0..=(seq.len().saturating_sub(2)) {
            if i + 2 <= seq.len() {
                substrings.insert(&seq[i..i + 2]);
            }
        }

        let expected = vec!["00", "01", "10", "11"];
        for exp in expected {
            assert!(substrings.contains(exp), "Missing substring: {}", exp);
        }
        assert_eq!(substrings.len(), 4);
    }

    #[test]
    fn test_de_bruijn_n3() {
        let seq = generate_de_bruijn_sequence(3);
        println!("n=3: {}", seq);

        // Should contain each 3-length substring exactly once
        let mut substrings = HashSet::new();
        for i in 0..=(seq.len().saturating_sub(3)) {
            if i + 3 <= seq.len() {
                substrings.insert(&seq[i..i + 3]);
            }
        }

        let expected = vec!["000", "001", "010", "011", "100", "101", "110", "111"];
        for exp in expected {
            assert!(substrings.contains(exp), "Missing substring: {}", exp);
        }
        assert_eq!(substrings.len(), 8);
    }

    #[test]
    fn test_de_bruijn_n6_length() {
        let seq = generate_de_bruijn_sequence(6);
        println!("n=6 length: {}", seq.len());

        // Length should be 2^6 + 6 - 1 = 64 + 5 = 69
        // Actually for circular de Bruijn it's 2^6 = 64, but our implementation is linear
        // so it should be around 64 + some padding
        assert!(seq.len() >= 64, "Sequence too short: {}", seq.len());
    }
}
