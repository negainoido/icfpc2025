use clap::Parser;

/// 2つの数値を加算するシンプルなプログラム
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// 1つ目の数値
    #[arg(short = 'a', long)]
    a: i32,

    /// 2つ目の数値
    #[arg(short = 'b', long)]
    b: i32,
}

fn main() {
    let args = Args::parse();
    let sum = args.a + args.b;
    println!("{}", sum);
}
