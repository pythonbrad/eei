mod predict;
use std::ffi::CString;
use std::os::raw::{c_char, c_int};
use std::{ptr, mem};
use std::mem::ManuallyDrop;

use predict::PREDICTOR;

#[repr(C)]
struct WordPredictions {
    len: c_int,
    words: *mut *mut c_char
}

#[repr(C)]
struct SymbolPredictions {
    len: c_int,
    symbols: *mut *mut c_char,
    shortcodes: *mut *mut c_char
}

#[no_mangle]
pub unsafe extern "C" fn get_word_predictions(characters: *mut c_char) -> WordPredictions {
    //TODO: error handling
    let context = CString::from_raw(characters).into_string().unwrap();
    let word_predictions = PREDICTOR.word(context.as_str()).unwrap();

    //TODO: return actual data
    let res = WordPredictions {
        len: word_predictions.len() as c_int,
        words: convert_string_vector(word_predictions)
    };
    let r = *res;
    mem::forget(res);
    r
}

#[no_mangle]
pub unsafe extern "C" fn free_word_predictions(predictions: WordPredictions) {
    free_string_array(predictions.words, predictions.len);
}


//based on
////https://play.rust-lang.org/?version=stable&mode=debug&edition=2018&gist=d0e44ce1f765ce89523ef89ccd864e54
fn convert_string_vector(str_vec: Vec<String>) -> *mut *mut c_char {
    //TODO: do we need to verify these strings before we convert them? (check for zero bytes inside)
    let mut cstring_vec: Vec<*mut c_char> = str_vec.into_iter().map(|s| {
        CString::from(s.into_bytes()).into_raw()
    }).collect();

    let len = cstring_vec.len() as c_int;
    let ptr = out.as_mut_ptr();
    mem::forget(out);

    ptr
}

unsafe fn free_string_array(ptr: *mut *mut c_char, len: c_int) {
    let len = len as usize;

    // Get back our vector.
    // Previously we shrank to fit, so capacity == length.
    let v = Vec::from_raw_parts(ptr, len, len);

    // Now drop one string at a time.
    for elem in v {
        let s = CString::from_raw(elem);
        mem::drop(s);
    }

    // Afterwards the vector will be dropped and thus freed.
}



