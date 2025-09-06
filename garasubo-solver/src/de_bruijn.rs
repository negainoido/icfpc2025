#![forbid(unsafe_code)]
//! ICFPC 2025 /explore 用プラン生成ライブラリ（de Bruijn ベース）
//!
//! 乱数を使わず 6 進 de Bruijn 列からサフィックスと追加プレフィックスを構成します。
//! 方式: fixed-tail fingerprinting（s∘R と s∘a∘R を非適応でまとめて投げる）
//!
//! # 使い方（例）
//! ```ignore
//! use icfpc_explore_planner::{PlannerConfig, generate_explore_plans};
//!
//! let cfg = PlannerConfig::default(); // デフォルトは 1,729 本
//! let plans = generate_explore_plans(&cfg);
//! assert!(!plans.is_empty());
//!
//! // 例えば n=30 問題で長さ制限を確認
//! let (max_len, within) = check_length_limit(&plans, 30);
//! assert!(within, "max plan length {max_len} exceeds 18n");
//! ```
//!
//! 必要に応じて `suffix_order` や `prefix_source_order`、`suffix_len`、`extra_prefix_*` を調整してください。

use std::collections::BTreeSet;

/// 生成パラメータ
#[derive(Clone, Debug)]
pub struct PlannerConfig {
    /// 全列挙する固定長プレフィックスの長さ L（例: 3）
    pub prefix_enum_len: usize,
    /// 追加プレフィックスの本数（de Bruijn 列から切り出し）
    pub extra_prefix_count: usize,
    /// 追加プレフィックスの最短長
    pub extra_prefix_min_len: usize,
    /// 追加プレフィックスの最長長（min..=max でラウンドロビン）
    pub extra_prefix_max_len: usize,
    /// 空プレフィックス ε を含めるか
    pub include_empty_prefix: bool,

    /// サフィックス R の長さ（de Bruijn 列の先頭から切り出す長さ）
    pub suffix_len: usize,
    /// サフィックスを 2 本にする（R1, R2。R2 は de Bruijn 周期の中点から）
    pub two_suffixes: bool,

    /// サフィックス用 de Bruijn 列の次数（6^order が周期長）
    pub suffix_order: usize,
    /// 追加プレフィックス抽出に用いる de Bruijn 列の次数
    pub prefix_source_order: usize,

    /// 末尾の a∈{0..5} を付けたバリアント（s∘a∘R）を作るか
    pub with_one_step_variants: bool,
}

impl Default for PlannerConfig {
    fn default() -> Self {
        Self {
            prefix_enum_len: 3,
            extra_prefix_count: 30,
            extra_prefix_min_len: 6,
            extra_prefix_max_len: 9,
            include_empty_prefix: true,

            suffix_len: 16,
            two_suffixes: false,

            // 周期長は 6^order。計算量/品質のバランスで既定は 4 と 5
            suffix_order: 4,        // 6^4 = 1296
            prefix_source_order: 5, // 6^5 = 7776

            with_one_step_variants: true,
        }
    }
}

pub fn config_for_rooms(n: usize) -> PlannerConfig {
    let mut cfg = PlannerConfig::default();
    match n {
        0..=8 => {
            // n=6 想定
            cfg.prefix_enum_len = 0;
            cfg.extra_prefix_count = 8;
            cfg.extra_prefix_min_len = 6;
            cfg.extra_prefix_max_len = 8;
            cfg.suffix_len = 12;
        }
        9..=14 => {
            // n=12 想定
            cfg.prefix_enum_len = 0;
            cfg.extra_prefix_count = 14;
            cfg.extra_prefix_min_len = 6;
            cfg.extra_prefix_max_len = 8;
            cfg.suffix_len = 12;
        }
        15..=20 => {
            // n=18 想定
            cfg.prefix_enum_len = 0;
            cfg.extra_prefix_count = 21;
            cfg.extra_prefix_min_len = 6;
            cfg.extra_prefix_max_len = 9;
            cfg.suffix_len = 13;
        }
        21..=26 => {
            // n=24 想定
            cfg.prefix_enum_len = 0;
            cfg.extra_prefix_count = 28;
            cfg.extra_prefix_min_len = 6;
            cfg.extra_prefix_max_len = 9;
            cfg.suffix_len = 14;
        }
        _ => {
            // n=30 など
            cfg.prefix_enum_len = 0;
            cfg.extra_prefix_count = 35;
            cfg.extra_prefix_min_len = 6;
            cfg.extra_prefix_max_len = 9;
            cfg.suffix_len = 16; // 14 でも可。16 は余裕枠
            cfg.two_suffixes = false;
        }
    }
    cfg
}

/// /explore の "plans" 配列を生成
///
/// - まずプレフィックス集合 S を構築（ε, 長さ L 全列挙, deBruijn から追加）
/// - サフィックス列 R（1 または 2 本）を生成
/// - 各 s∈S について s∘R と、必要なら s∘a∘R (a=0..5) を生成
pub fn generate_explore_plans(cfg: &PlannerConfig) -> Vec<String> {
    let prefixes = generate_prefixes(cfg);
    let suffixes = generate_suffixes(cfg);

    let mut plans = Vec::<String>::with_capacity(
        prefixes.len() * (1 + if cfg.with_one_step_variants { 6 } else { 0 }) * suffixes.len(),
    );

    for s in &prefixes {
        for r in &suffixes {
            // s ∘ R
            let mut p = String::with_capacity(s.len() + r.len());
            p.push_str(s);
            p.push_str(r);
            plans.push(p);

            // s ∘ a ∘ R
            if cfg.with_one_step_variants {
                for a in 0..6 {
                    let mut q = String::with_capacity(s.len() + 1 + r.len());
                    q.push_str(s);
                    q.push(char::from(b'0' + a as u8));
                    q.push_str(r);
                    plans.push(q);
                }
            }
        }
    }
    plans
}

/// プレフィックス集合 S を生成（ε / 長さ L 全列挙 / de Bruijn から追加）
pub fn generate_prefixes(cfg: &PlannerConfig) -> Vec<String> {
    assert!(
        cfg.extra_prefix_min_len >= 1,
        "extra_prefix_min_len must be >= 1"
    );
    assert!(
        cfg.extra_prefix_min_len <= cfg.extra_prefix_max_len,
        "extra_prefix_min_len must be <= extra_prefix_max_len"
    );

    let mut set: BTreeSet<String> = BTreeSet::new();

    if cfg.include_empty_prefix {
        set.insert(String::new());
    }

    // 長さ L の全列挙（6^L 個）
    if cfg.prefix_enum_len > 0 {
        for s in enumerate_base6_fixed_len(cfg.prefix_enum_len) {
            set.insert(s);
        }
    }

    // de Bruijn（order = prefix_source_order）から等間隔で開始点を取り、
    // 長さは [min, max] をラウンドロビンで割り当てて切り出す
    if cfg.extra_prefix_count > 0 {
        let db = de_bruijn_base6(cfg.prefix_source_order);
        let m = db.len();
        let span = cfg.extra_prefix_max_len - cfg.extra_prefix_min_len + 1;

        for i in 0..cfg.extra_prefix_count {
            let start = (i * m) / cfg.extra_prefix_count; // 等間隔
            let len = cfg.extra_prefix_min_len + (i % span);
            let s = substring_from_cycle(&db, start, len);
            set.insert(digits_to_string(&s));
        }
    }

    set.into_iter().collect()
}

/// サフィックス列（R を 1 または 2 本）を生成
///
/// - R1: de Bruijn(order = cfg.suffix_order) の先頭から cfg.suffix_len 文字
/// - R2: 周期の中点から cfg.suffix_len 文字（two_suffixes=true のとき）
pub fn generate_suffixes(cfg: &PlannerConfig) -> Vec<String> {
    assert!(cfg.suffix_len >= 1, "suffix_len must be >= 1");
    let db = de_bruijn_base6(cfg.suffix_order);
    let period = db.len();

    let r1 = digits_to_string(&substring_from_cycle(&db, 0, cfg.suffix_len));

    if cfg.two_suffixes {
        let start2 = period / 2; // 中点から
        let r2digits = substring_from_cycle(&db, start2, cfg.suffix_len);
        let r2 = digits_to_string(&r2digits);
        if r2 == r1 {
            // 万一同一になった場合は 1 文字ずらす
            let r2alt = digits_to_string(&substring_from_cycle(
                &db,
                (start2 + 1) % period,
                cfg.suffix_len,
            ));
            return vec![r1, r2alt];
        }
        vec![r1, r2]
    } else {
        vec![r1]
    }
}

/// 6進 de Bruijn 列 B(6, order) を返す（長さ 6^order）
///
/// 実装は FKM アルゴリズム（Prefer-same / db(t,p) 再帰）に基づく。返り値は 0..5 の数字ベクタ。
pub fn de_bruijn_base6(order: usize) -> Vec<u8> {
    de_bruijn(6, order).into_iter().map(|x| x as u8).collect()
}

/// 一般 k進 de Bruijn B(k, n)。返り値は 0..k-1 の usize。
pub fn de_bruijn(k: usize, n: usize) -> Vec<usize> {
    assert!(k >= 2, "alphabet size k must be >= 2");
    assert!(n >= 1, "order n must be >= 1");

    // FKM (Fredricksen-Kessler-Maiorana) algorithm
    let mut a = vec![0usize; k * n];
    let mut seq = Vec::<usize>::with_capacity(k.pow(n as u32));
    fn db(t: usize, p: usize, k: usize, n: usize, a: &mut [usize], seq: &mut Vec<usize>) {
        if t > n {
            if n % p == 0 {
                for i in 1..=p {
                    seq.push(a[i]);
                }
            }
        } else {
            a[t] = a[t - p];
            db(t + 1, p, k, n, a, seq);
            for j in a[t - p] + 1..k {
                a[t] = j;
                db(t + 1, t, k, n, a, seq);
            }
        }
    }
    db(1, 1, k, n, &mut a, &mut seq);

    // 典型的には "0…0" を含む回転になっているが、厳密に先頭を 0^n にしたい場合のローテートも可能。
    // ここではローテートせず返す（利用側で substring_from_cycle するため問題なし）。
    seq
}

/// base-6 の固定長列挙（長さ len, 6^len 個）。'0'..'5' の文字列を辞書順で返す。
pub fn enumerate_base6_fixed_len(len: usize) -> Vec<String> {
    assert!(len >= 1, "len must be >= 1");
    let total = 6usize.pow(len as u32);
    let mut out = Vec::<String>::with_capacity(total);

    for mut v in 0..total {
        let mut buf = vec![b'0'; len];
        for i in (0..len).rev() {
            buf[i] = b'0' + (v % 6) as u8;
            v /= 6;
        }
        // 安全: 0..5 を '0'..'5' にマップ
        out.push(String::from_utf8(buf).unwrap());
    }
    out
}

/// 周期列（de Bruijn など）から [start, start+len) を循環で切り出す
pub fn substring_from_cycle(cycle: &[u8], start: usize, len: usize) -> Vec<u8> {
    assert!(!cycle.is_empty(), "cycle must be non-empty");
    assert!(len >= 1, "len must be >= 1");
    let m = cycle.len();
    let mut out = Vec::<u8>::with_capacity(len);
    let mut idx = start % m;
    for _ in 0..len {
        out.push(cycle[idx]);
        idx += 1;
        if idx == m {
            idx = 0;
        }
    }
    out
}

/// 0..5 の数字配列を "0".."5" の文字列へ
pub fn digits_to_string(digits: &[u8]) -> String {
    let mut s = String::with_capacity(digits.len());
    for &d in digits {
        debug_assert!(d < 6);
        s.push(char::from(b'0' + d));
    }
    s
}

/// /explore の各プラン長が 18n を超えていないかを確認
///
/// 戻り値: (最大長, 18n 以下か)
pub fn check_length_limit(plans: &[String], n: usize) -> (usize, bool) {
    let max_len = plans.iter().map(|p| p.len()).max().unwrap_or(0);
    let limit = 18usize * n;
    (max_len, max_len <= limit)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_debruijn_lengths() {
        for n in 1..=6 {
            let db = de_bruijn_base6(n);
            assert_eq!(db.len(), 6usize.pow(n as u32));
            // 値域チェック
            assert!(db.iter().all(|&x| x < 6));
        }
    }

    #[test]
    fn test_substring_cycle() {
        let cyc = vec![0u8, 1, 2, 3, 4, 5];
        let s = substring_from_cycle(&cyc, 4, 5);
        assert_eq!(s, vec![4, 5, 0, 1, 2]);
    }

    #[test]
    fn test_default_plan_count_and_lengths() {
        let cfg = PlannerConfig::default();
        let plans = generate_explore_plans(&cfg);
        // 既定: 空 + 6^3(=216) + 追加30 = 247 プレフィックス
        // 各 s につき 1 + 6 = 7、サフィックス1本 → 247*7=1729
        assert_eq!(plans.len(), 247 * 7);

        // 既定では suffix_len=16, prefix_enum_len=3 → 最大長は 3+1+16=20
        let (max_len, within) = check_length_limit(&plans, 6);
        assert_eq!(max_len, 20);
        assert!(within); // 18*6=108
    }

    #[test]
    fn test_two_suffixes() {
        let mut cfg = PlannerConfig::default();
        cfg.two_suffixes = true;
        let suffixes = generate_suffixes(&cfg);
        assert_eq!(suffixes.len(), 2);
        assert_ne!(suffixes[0], suffixes[1]);
    }

    #[test]
    fn test_prefixes_dedup_and_order() {
        let mut cfg = PlannerConfig::default();
        cfg.include_empty_prefix = true;
        let prefixes = generate_prefixes(&cfg);
        // 辞書順 (BTreeSet) で ε が先頭、その後 '000'.. '555'...
        assert_eq!(prefixes.first().unwrap(), "");
        assert_eq!(prefixes[1].len(), 3);
        assert_eq!(prefixes[1], "000");
    }
}
