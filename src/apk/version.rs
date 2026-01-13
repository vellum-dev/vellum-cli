use std::cmp::Ordering;

pub fn compare_versions(a: &str, b: &str) -> Ordering {
    if a == b {
        return Ordering::Equal;
    }

    let a_parts: Vec<&str> = a.split('.').collect();
    let b_parts: Vec<&str> = b.split('.').collect();

    let min_len = a_parts.len().min(b_parts.len());

    for i in 0..min_len {
        let a_num: i32 = a_parts[i]
            .split('-')
            .next()
            .unwrap_or("0")
            .parse()
            .unwrap_or(0);
        let b_num: i32 = b_parts[i]
            .split('-')
            .next()
            .unwrap_or("0")
            .parse()
            .unwrap_or(0);

        match a_num.cmp(&b_num) {
            Ordering::Greater => return Ordering::Greater,
            Ordering::Less => return Ordering::Less,
            Ordering::Equal => continue,
        }
    }

    a_parts.len().cmp(&b_parts.len())
}

pub fn version_gte(a: &str, b: &str) -> bool {
    compare_versions(a, b) != Ordering::Less
}

pub fn version_lt(a: &str, b: &str) -> bool {
    compare_versions(a, b) == Ordering::Less
}
