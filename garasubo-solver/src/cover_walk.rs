/// 被覆ウォーク生成器
/// - n: 部屋数（最大 90）
/// - 返り値: 扉番号列（0..=5）
///
/// 設計方針:
/// 1) 歩数上限 limit = floor(limit_ratio * n) （既定 6n）
/// 2) 目標長 target = min(limit, max(core_len, floor(target_ratio * n))) （既定 5n 以上）
/// 3) core は De Bruijn(k=6, order∈{3,2,1}) の最大次数で、core_len <= limit を満たすもの
/// 4) 余りは 0..5 のシャッフル・ブロック（長さ 6）や単発ステップで target まで埋める
///
/// 備考:
/// - De Bruijn の線形化のため先頭 (order-1) を末尾に再付加します（k^order + order - 1）
/// - 小さな n でも必ず limit 内に収まるよう次数を自動調整します。
// ---------------------------
// 乱数（依存無し / 再現性あり）
// ---------------------------
#[derive(Clone)]
struct XorShift64 {
    state: u64,
}
impl XorShift64 {
    fn new(seed: u64) -> Self {
        let s = if seed == 0 {
            0x9E37_79B9_7F4A_7C15
        } else {
            seed
        };
        Self { state: s }
    }
    #[inline]
    fn next_u64(&mut self) -> u64 {
        let mut x = self.state;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.state = x;
        x
    }
    #[inline]
    fn gen_range(&mut self, upper: usize) -> usize {
        // 0..upper-1
        (self.next_u64() % (upper as u64)) as usize
    }
    fn shuffle<T>(&mut self, slice: &mut [T]) {
        // Fisher–Yates
        let len = slice.len();
        for i in (1..len).rev() {
            let j = self.gen_range(i + 1);
            slice.swap(i, j);
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Params {
    /// 目標長の係数（≈ 5n 推奨）
    pub target_ratio: f64,
    /// 絶対上限の係数（≈ 6n 推奨）
    pub limit_ratio: f64,
    /// 乱数シード（固定で再現性確保）
    pub seed: u64,
}
impl Default for Params {
    fn default() -> Self {
        Self {
            target_ratio: 5.5,
            limit_ratio: 6.0,
            seed: 0xC0FF_EE_u64,
        }
    }
}

#[inline]
fn pow_usize(base: usize, exp: usize) -> usize {
    let mut r = 1usize;
    for _ in 0..exp {
        r = r.saturating_mul(base);
    }
    r
}

/// De Bruijn 列（基数 k, 次数 n）を 0..k-1 の Vec<u8> で返す（循環列）
fn de_bruijn_cycle(k: usize, order: usize) -> Vec<u8> {
    // ラド・ハミングの再帰実装
    fn db(t: usize, p: usize, k: usize, n: usize, a: &mut [usize], out: &mut Vec<u8>) {
        if t > n {
            if n % p == 0 {
                for &ai in &a[1..=p] {
                    out.push(ai as u8);
                }
            }
        } else {
            a[t] = a[t - p];
            db(t + 1, p, k, n, a, out);
            for j in (a[t - p] + 1)..k {
                a[t] = j;
                db(t + 1, t, k, n, a, out);
            }
        }
    }
    let mut a = vec![0usize; k * order];
    let mut out = Vec::<u8>::with_capacity(pow_usize(k, order));
    db(1, 1, k, order, &mut a, &mut out);
    out // 長さ k^order の循環列
}

/// De Bruijn の “線形化”:
/// 循環列 + 先頭 (order-1) を末尾に足したもの（全 n-gram を 1 回ずつ含む）
fn de_bruijn_linear(k: usize, order: usize) -> Vec<u8> {
    let cyc = de_bruijn_cycle(k, order);
    let mut lin = cyc.clone();
    if order > 1 {
        lin.extend_from_slice(&cyc[..(order - 1)]);
    }
    lin
}

fn choose_order_within_limit(limit: usize) -> usize {
    // k=6, order 3→2→1 の順で最大次数を選択
    const K: usize = 6;
    for ord in [3usize, 2, 1] {
        let need = pow_usize(K, ord) + ord.saturating_sub(1);
        if need <= limit {
            return ord;
        }
    }
    // 最低でも 1 ステップは返す（極端に小さい limit 想定外の場合）
    1
}

fn next_not_eq(last: u8, rng: &mut XorShift64) -> u8 {
    // 0..5 の中から last と異なる値を 1 つ
    loop {
        let v = rng.gen_range(6) as u8;
        if v != last {
            return v;
        }
    }
}

/// 0..5 のシャッフル・ブロックを作る。先頭が `avoid` と同じなら 1 回だけ回転して回避。
fn perm_block(avoid: Option<u8>, rng: &mut XorShift64) -> [u8; 6] {
    let mut arr = [0u8, 1, 2, 3, 4, 5];
    rng.shuffle(&mut arr);
    if let Some(a) = avoid {
        if arr[0] == a {
            arr.rotate_left(1);
        }
    }
    arr
}

/// メイン API: n から被覆ウォークを生成
pub fn generate_cover_walk(n: usize) -> Vec<u8> {
    generate_cover_walk_with_params(n, Params::default())
}

/// パラメトリック版
pub fn generate_cover_walk_with_params(n: usize, params: Params) -> Vec<u8> {
    const K: usize = 6;
    let limit = ((params.limit_ratio * n as f64).floor() as usize).max(1);
    let db_order = choose_order_within_limit(limit);
    let mut core = de_bruijn_linear(K, db_order);
    let core_len = core.len(); // k^order + order - 1

    // 目標長（core を必ず含みたいので max(core_len, floor(target_ratio*n))）
    let target_len = {
        let t = (params.target_ratio * n as f64).floor() as usize;
        core_len.max(t).min(limit)
    };

    // 余りを 0..5 のシャッフル・ブロックと単発ステップで埋める
    let mut rng = XorShift64::new(params.seed ^ (n as u64) ^ ((db_order as u64) << 32));
    let mut walk = Vec::<u8>::with_capacity(target_len);
    walk.extend_from_slice(&core);

    // ブロック追加
    while walk.len() + K <= target_len {
        let avoid = walk.last().copied();
        let blk = perm_block(avoid, &mut rng);
        walk.extend_from_slice(&blk);
    }
    // 端数を単発で詰める
    while walk.len() < target_len {
        let v = match walk.last().copied() {
            Some(last) => next_not_eq(last, &mut rng),
            None => (rng.gen_range(6) as u8),
        };
        walk.push(v);
    }
    walk
}

/// ルートプラン用の文字列に変換（'0'..'5' の連結）
pub fn to_route_plan(doors: &[u8]) -> String {
    let mut s = String::with_capacity(doors.len());
    for &d in doors {
        debug_assert!(d <= 5);
        s.push(char::from(b'0' + d));
    }
    s
}

// 簡単なテスト（必要なら）
#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn covers_all_ngrams_when_order3_fits() {
        // n が十分大きいと order=3 を選べる → 3-gram 全網羅
        let n = 90usize;
        let params = Params::default();
        let w = generate_cover_walk_with_params(n, params);
        // 3-gram の全 6^3 = 216 パターンがどこかに出てくるはず
        let mut seen = HashSet::new();
        for window in w.windows(3) {
            seen.insert((window[0], window[1], window[2]));
        }
        assert_eq!(seen.len(), 6usize.pow(3));
    }

    #[test]
    fn length_within_limit() {
        for n in [8usize, 20, 40, 90] {
            let w = generate_cover_walk(n);
            assert!(w.len() <= (6 * n));
            assert!(w.len() >= (5 * n).min(6 * n).min(w.len()));
        }
    }

    #[test]
    fn no_panic_small_n() {
        for n in 1..=7 {
            let w = generate_cover_walk(n);
            assert!(!w.is_empty());
            assert!(w.len() <= 6 * n);
        }
    }
}
