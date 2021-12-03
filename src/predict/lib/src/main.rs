mod predict;

use predict::PREDICTOR;

fn main() {
    let word_pref = "lit";

    let symbol_results = PREDICTOR.symbol("eq").unwrap();
    let word_results = PREDICTOR.word(word_pref).unwrap();
    println!("symbols for eq");
    for (shortcode, symbol) in symbol_results {
        println!("{shortcode} : {symbol}", shortcode=shortcode, symbol=symbol);
    }
    println!("words for {pref}:", pref=word_pref);
    for word in word_results {
        println!("{word}", word=word);
    }

}