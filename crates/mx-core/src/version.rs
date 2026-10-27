use std::cmp::Ordering;

fn parts(v: &str) -> Vec<i64> {
    let v = v.strip_prefix('v').unwrap_or(v);
    v.split(['.', '+', '-']).map(|p| p.parse::<i64>().unwrap_or(0)).collect()
}

pub fn compare_versions(a: &str, b: &str) -> Ordering {
    let (x, y) = (parts(a), parts(b));
    let n = x.len().max(y.len());
    (0..n)
        .map(|i| x.get(i).copied().unwrap_or(0).cmp(&y.get(i).copied().unwrap_or(0)))
        .find(|o| o.is_ne())
        .unwrap_or(Ordering::Equal)
}
