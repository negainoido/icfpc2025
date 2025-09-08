// signature_index.rs
//
// Phase A: 前処理＆署名索引の実装
//
// 入力：
//   - W: 扉列（長さ L、各要素 0..=5）
//   - Y: ラベル列（長さ L+1、各要素 0..=3）
// 出力：
//   - SigIndex: 署名（u64 キー）→ 時刻インデックス（u32）のベクタ
//
// 使い所：Phase B 以降の「候補生成（シグネチャ一致で候補ペア抽出）」にそのまま渡せます。

use std::collections::HashMap;

//-----------------------------
// 公開インターフェース
//-----------------------------

/// 署名索引（Signature → Vec<time_index>）
/// - 各 Vec<u32> は「その署名を満たす時刻 t（= 状態 R_t）」の集合
/// - f1 は t ∈ [0, L-1]、b1 は t ∈ [1, L]、f2 は t ∈ [0, L-2]、b2 は t ∈ [2, L]、mix は t ∈ [1, L-1]
#[derive(Debug, Default)]
pub struct SigIndex {
    pub f1: HashMap<u64, Vec<u32>>,
    pub b1: HashMap<u64, Vec<u32>>,
    pub f2: HashMap<u64, Vec<u32>>,
    pub b2: HashMap<u64, Vec<u32>>,
    pub mix: Option<HashMap<u64, Vec<u32>>>, // パラメータで有効化
}

/// 構築パラメータ
#[derive(Debug, Clone, Copy)]
pub struct BuildParams {
    /// 巨大バケット抑制用の上限（署名キーごとの最大件数）。None なら無制限。
    pub bucket_cap: Option<usize>,
    /// mix 署名（前後混合 5-tuple）を作るか
    pub enable_mix: bool,
    /// 乱数シード（バケット制限のサンプリングに使用）
    pub seed: u64,
}
impl Default for BuildParams {
    fn default() -> Self {
        Self {
            bucket_cap: Some(128),   // 例：各署名キーにつき最大 50 件まで保持
            enable_mix: true,
            seed: 0x5EED_C0DE_1234_ABCD,
        }
    }
}

/// 入力不正時のエラー
#[derive(Debug, thiserror::Error)]
pub enum BuildError {
    #[error("Length mismatch: w_len={w_len}, y_len={y_len}")]
    LengthMismatch { w_len: usize, y_len: usize },
    #[error("Door out of range: index={index}, value={value}")]
    DoorOutOfRange { index: usize, value: u8 },
    #[error("Label out of range: index={index}, value={value}")]
    LabelOutOfRange { index: usize, value: u8 },
}

/// メイン API：W, Y から署名索引を構築
pub fn build_signature_index(
    w: &[u8], // 0..=5
    y: &[u8], // 0..=3、長さは w.len()+1
    params: BuildParams,
) -> Result<SigIndex, BuildError> {
    validate_inputs(w, y)?;

    let l = w.len();
    let mut f1: HashMap<u64, Vec<u32>> = HashMap::new();
    let mut b1: HashMap<u64, Vec<u32>> = HashMap::new();
    let mut f2: HashMap<u64, Vec<u32>> = HashMap::new();
    let mut b2: HashMap<u64, Vec<u32>> = HashMap::new();
    let mut mix: Option<HashMap<u64, Vec<u32>>> = if params.enable_mix {
        Some(HashMap::new())
    } else {
        None
    };

    // ---- f1: (Y[t], W[t], Y[t+1]) for t in [0, L-1]
    for t in 0..l {
        let key = pack3(y[t], w[t], y[t + 1]);
        f1.entry(key).or_default().push(t as u32);
    }

    // ---- b1: (Y[t-1], W[t-1], Y[t]) for t in [1, L]
    for t in 1..=l {
        let key = pack3(y[t - 1], w[t - 1], y[t]);
        b1.entry(key).or_default().push(t as u32);
    }

    // ---- f2: (Y[t], W[t], Y[t+1], W[t+1], Y[t+2]) for t in [0, L-2]
    if l >= 2 {
        for t in 0..=(l - 2) {
            let key = pack5(y[t], w[t], y[t + 1], w[t + 1], y[t + 2]);
            f2.entry(key).or_default().push(t as u32);
        }
    }

    // ---- b2: (Y[t-2], W[t-2], Y[t-1], W[t-1], Y[t]) for t in [2, L]
    if l >= 2 {
        for t in 2..=l {
            let key = pack5(y[t - 2], w[t - 2], y[t - 1], w[t - 1], y[t]);
            b2.entry(key).or_default().push(t as u32);
        }
    }

    // ---- mix: (Y[t-1], W[t-1], Y[t], W[t], Y[t+1]) for t in [1, L-1]
    if params.enable_mix && l >= 1 {
        let m = mix.as_mut().unwrap();
        for t in 1..l {
            let key = pack5(y[t - 1], w[t - 1], y[t], w[t], y[t + 1]);
            m.entry(key).or_default().push(t as u32);
        }
    }

    // ---- バケット上限によるダウンサンプリング
    if let Some(cap) = params.bucket_cap {
        let mut rng = XorShift64::new(params.seed ^ (l as u64));
        cap_buckets(&mut f1, cap, &mut rng);
        cap_buckets(&mut b1, cap, &mut rng);
        cap_buckets(&mut f2, cap, &mut rng);
        cap_buckets(&mut b2, cap, &mut rng);
        if let Some(m) = mix.as_mut() {
            cap_buckets(m, cap, &mut rng);
        }
    }

    Ok(SigIndex { f1, b1, f2, b2, mix })
}

//-----------------------------
// 内部：入力検証・キー生成・ユーティリティ
//-----------------------------

fn validate_inputs(w: &[u8], y: &[u8]) -> Result<(), BuildError> {
    if y.len() != w.len() + 1 {
        return Err(BuildError::LengthMismatch {
            w_len: w.len(),
            y_len: y.len(),
        });
    }
    for (i, &d) in w.iter().enumerate() {
        if d > 5 {
            return Err(BuildError::DoorOutOfRange { index: i, value: d });
        }
    }
    for (i, &r) in y.iter().enumerate() {
        if r > 3 {
            return Err(BuildError::LabelOutOfRange { index: i, value: r });
        }
    }
    Ok(())
}

// 2bit（ラベル）と3bit（扉）で u64 キーにパック
#[inline]
fn pack3(y0: u8, d0: u8, y1: u8) -> u64 {
    const LM: u64 = (1 << 2) - 1; // 0b11
    const DM: u64 = (1 << 3) - 1; // 0b111
    let a = (y0 as u64) & LM;
    let b = ((d0 as u64) & DM) << 2;
    let c = ((y1 as u64) & LM) << (2 + 3);
    a | b | c // 7 ビット使用
}
#[inline]
fn pack5(y0: u8, d0: u8, y1: u8, d1: u8, y2: u8) -> u64 {
    const LM: u64 = (1 << 2) - 1;
    const DM: u64 = (1 << 3) - 1;
    let a = (y0 as u64) & LM;               // +0..1
    let b = ((d0 as u64) & DM) << 2;        // +2..4
    let c = ((y1 as u64) & LM) << 5;        // +5..6
    let d = ((d1 as u64) & DM) << 7;        // +7..9
    let e = ((y2 as u64) & LM) << 10;       // +10..11
    a | b | c | d | e // 12 ビット使用
}

// バケット上限：Vec をインプレースでシャッフル→truncate
fn cap_buckets(map: &mut HashMap<u64, Vec<u32>>, cap: usize, rng: &mut XorShift64) {
    for v in map.values_mut() {
        if v.len() > cap {
            fisher_yates_shuffle(v, rng);
            v.truncate(cap);
        }
    }
}
fn fisher_yates_shuffle<T>(arr: &mut [T], rng: &mut XorShift64) {
    let n = arr.len();
    for i in (1..n).rev() {
        let j = rng.gen_range(i + 1);
        arr.swap(i, j);
    }
}

// 依存なしの軽量 PRNG（再現性あり）
#[derive(Clone)]
struct XorShift64 {
    state: u64,
}
impl XorShift64 {
    fn new(seed: u64) -> Self {
        let s = if seed == 0 { 0x9E37_79B9_7F4A_7C15 } else { seed };
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
        (self.next_u64() % (upper as u64)) as usize
    }
}

//-----------------------------
// 使い方サンプル（テスト）
//-----------------------------
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_ok_and_ranges() {
        // 例の小さなウォーク（L=6）
        // W: 0..5 の列、Y: 0..3 の列（長さは L+1）
        let w = [0u8, 1, 2, 3, 4, 5];
        let y = [1u8, 0, 1, 1, 2, 3, 0];

        let idx = build_signature_index(&w, &y, BuildParams::default()).unwrap();
        // f1 は L=6 件、b1 は L=6 件、f2/b2 は L-1=5 件、mix は L-1=5 件（enable_mix=true）
        assert_eq!(count_total(&idx.f1), 6);
        assert_eq!(count_total(&idx.b1), 6);
        assert_eq!(count_total(&idx.f2), 5);
        assert_eq!(count_total(&idx.b2), 5);
        assert!(idx.mix.is_some());
        assert_eq!(count_total(idx.mix.as_ref().unwrap()), 5);
    }

    #[test]
    fn bucket_cap_applies() {
        // すべて同じ署名に落ちるように Y を揃える（バケットが大きくなる）
        let l = 120usize;
        let mut w = vec![0u8; l];
        let mut y = vec![0u8; l + 1];
        for i in 0..l {
            w[i] = (i % 6) as u8;
            y[i] = (i % 4) as u8;
        }
        y[l] = 0;

        let params = BuildParams {
            bucket_cap: Some(8),
            enable_mix: true,
            seed: 42,
        };
        let idx = build_signature_index(&w, &y, params).unwrap();

        // どの署名マップでも、各キーの Vec 長は高々 8 になる
        assert!(max_bucket(&idx.f1) <= 8);
        assert!(max_bucket(&idx.b1) <= 8);
        assert!(max_bucket(&idx.f2) <= 8);
        assert!(max_bucket(&idx.b2) <= 8);
        assert!(max_bucket(idx.mix.as_ref().unwrap()) <= 8);
    }

    fn count_total(map: &HashMap<u64, Vec<u32>>) -> usize {
        map.values().map(|v| v.len()).sum()
    }
    fn max_bucket(map: &HashMap<u64, Vec<u32>>) -> usize {
        map.values().map(|v| v.len()).max().unwrap_or(0)
    }
}