#![allow(non_upper_case_globals)]
mod predict;

use std::ffi::{CString, NulError, CStr};
use std::os::raw::{c_char, c_int};
use log::LevelFilter;
use log4rs::append::file::FileAppender;
use log4rs::encode::pattern::PatternEncoder;
use log4rs::config::{Appender, Config, Root};

use crate::predict::PREDICTOR;
use ibus::{IBusEEIEngine, gboolean, GBOOL_FALSE, ibus_engine_update_lookup_table, IBusEngine, GBOOL_TRUE, ibus_engine_hide_lookup_table, guint, IBusModifierType_IBUS_CONTROL_MASK, IBUS_e, IBUS_w, IBUS_asciitilde, IBUS_space, IBUS_Return, IBUS_BackSpace, IBUS_Escape, IBUS_Page_Down, IBUS_Page_Up, ibus_engine_commit_text, ibus_text_new_from_unichar, ibus_text_new_from_string, gchar, ibus_lookup_table_clear, ibus_lookup_table_append_candidate, IBusText, ibus_engine_update_auxiliary_text, IBUS_Up, IBUS_Down, ibus_lookup_table_get_cursor_pos, IBusLookupTable, ibus_lookup_table_get_label, ibus_lookup_table_cursor_up, ibus_lookup_table_cursor_down, ibus_engine_hide_auxiliary_text, ibus_lookup_table_set_label, ibus_lookup_table_page_down, ibus_lookup_table_page_up, ibus_lookup_table_get_number_of_candidates, ibus_text_new_from_static_string, ibus_engine_show_lookup_table, ibus_lookup_table_get_cursor_in_page, gunichar, IBusModifierType_IBUS_SHIFT_MASK, ibus_lookup_table_get_candidate};
use std::cmp::min;
use lazy_static::lazy_static;

lazy_static! {
    static ref empty_cstring: CString = CString::new("").unwrap();
}

pub struct EngineCore {
    lookup_visible: bool,
    word_buffer: String,
    symbol_input: bool,
    symbol_preedit: String,
    symbol_label_vec: Vec<CString>,
    symbol_last_page: guint,
    parent_engine: *mut IBusEEIEngine
}

#[no_mangle]
pub unsafe extern "C" fn new_engine_core(parent_engine: *mut IBusEEIEngine) -> *mut EngineCore {
    Box::into_raw(Box::new(EngineCore {
        lookup_visible: false,
        word_buffer: String::new(),
        symbol_input: false,
        symbol_preedit: String::new(),
        symbol_label_vec: Vec::new(),
        symbol_last_page: 0,
        parent_engine: parent_engine
    }))
}

unsafe fn into_ibus_string(input: String) -> Result<*mut IBusText, NulError> {
    CString::new(input.into_bytes()).map(|cstr| ibus_text_new_from_string(cstr.into_raw() as *const gchar))
}

impl EngineCore {

    fn parent_engine_as_ibus_engine(&self) -> *mut IBusEngine {
        self.parent_engine as *mut IBusEngine
    }

    unsafe fn get_table(&self) -> *mut IBusLookupTable {
        (*self.parent_engine).table
    }

    unsafe fn get(engine: *mut IBusEngine) -> Option<&'static mut EngineCore> {
        ((*(engine as *mut IBusEEIEngine)).engine_core as *mut EngineCore).as_mut()
    }

    unsafe fn update_lookup_table(&mut self) {
        if self.symbol_input {
            let page_size = (*self.get_table()).page_size;
            let idx = ibus_lookup_table_get_cursor_pos(self.get_table());
            let page_num = idx / page_size;
            if self.symbol_last_page != page_num {
                self.symbol_last_page = page_num;
                for (idx, table_idx) in (page_num * page_size..min(page_size * (page_size+1), self.symbol_label_vec.len() as u32)).enumerate() {
                    ibus_lookup_table_set_label(self.get_table(), idx as guint, ibus_text_new_from_static_string(self.symbol_label_vec.get_unchecked(table_idx as usize).as_ptr()))
                }
            }
        }
        ibus_engine_update_lookup_table(self.parent_engine_as_ibus_engine(), self.get_table(), GBOOL_TRUE);
    }

    unsafe fn word_table_update(&mut self) -> gboolean {
        if !self.lookup_visible || self.symbol_input {
            return GBOOL_FALSE;
        }
        else if self.word_buffer.is_empty() {
            return self.word_table_disable();
        }

        let search_result  = PREDICTOR.word(self.word_buffer.as_str());
        match search_result {
            Ok(candidates) => {
                log::info!("Word search for {} and got {:?}", self.word_buffer, candidates);
                let table = self.get_table();
                ibus_lookup_table_clear(table);
                for word in candidates {
                    match CString::new(word.into_bytes()) {
                        Ok(cstring_word) => {
                            ibus_lookup_table_append_candidate(table, ibus_text_new_from_string(cstring_word.into_raw() as *mut gchar));
                        }
                        Err(err) => {
                            log::error!("Failed string conversion for word lookup: {}", err);
                        }
                    }
                }
                ibus_engine_update_lookup_table(self.parent_engine_as_ibus_engine(), table, GBOOL_TRUE);
                GBOOL_TRUE
            }
            Err(err) => {
                log::error!("{}", err);
                GBOOL_FALSE
            }
        }
    }

    unsafe fn word_table_enable(&mut self) -> gboolean {
        if self.lookup_visible || self.word_buffer.is_empty() {
            return GBOOL_FALSE;
        }

        self.lookup_visible = true;
        let ret = self.word_table_update();
        ibus_lookup_table_clear(self.get_table());
        ibus_engine_show_lookup_table(self.parent_engine_as_ibus_engine());
        ret
    }

    unsafe fn word_table_disable(&mut self) -> gboolean {
        if !self.lookup_visible {
            return GBOOL_FALSE;
        }

        self.lookup_visible = false;
        ibus_engine_hide_lookup_table(self.parent_engine_as_ibus_engine());
        GBOOL_TRUE
    }

    unsafe fn symbol_input_enable(&mut self) -> gboolean {
        if self.lookup_visible {
            return GBOOL_FALSE;
        }

        self.symbol_input = true;
        self.lookup_visible = true;
        ibus_lookup_table_clear(self.get_table());
        ibus_engine_show_lookup_table(self.parent_engine_as_ibus_engine());
        GBOOL_TRUE
    }

    unsafe fn symbol_input_disable(&mut self) -> gboolean {
        if !self.symbol_input {
            return GBOOL_FALSE;
        }

        self.symbol_input = false;
        self.lookup_visible = false;
        self.symbol_preedit.clear();
        ibus_engine_hide_lookup_table(self.parent_engine_as_ibus_engine());
        ibus_engine_hide_auxiliary_text(self.parent_engine_as_ibus_engine());
        for i in 0..(*self.get_table()).page_size {
            ibus_lookup_table_set_label(self.get_table(), i, ibus_text_new_from_static_string(empty_cstring.as_ptr()));
        }
        GBOOL_TRUE
    }

    unsafe fn commit_char(&mut self, keyval: guint) -> gboolean {
        self.word_buffer.push((keyval as u8) as char);
        ibus_engine_commit_text(self.parent_engine_as_ibus_engine(), ibus_text_new_from_unichar(keyval as gunichar));
        GBOOL_TRUE
    }

    unsafe fn symbol_input_update(&mut self) -> gboolean {
        match into_ibus_string(self.symbol_preedit.clone()) {
            Ok(ibus_string) => {
                ibus_engine_update_auxiliary_text(self.parent_engine_as_ibus_engine(), ibus_string, GBOOL_TRUE);
            }
            Err(err) => {
                log::error!("Failed string conversion for symbol aux text update: {}", err);
            }
        }

        if self.symbol_preedit.is_empty() {
            return GBOOL_TRUE;
        }

        let search_result  = PREDICTOR.symbol(self.symbol_preedit.as_str());
        match search_result {
            Ok(candidates) => {
                log::info!("Symbol search for {} and got {:?}", self.symbol_preedit, candidates);
                let table = self.get_table();
                // Must clear table first, since the table may have IBusText referencing the
                // symbol_label_vec strings
                ibus_lookup_table_clear(table);
                self.symbol_label_vec.clear();
                for (idx, (shortcode, ident)) in candidates.into_iter().enumerate() {
                    match (CString::new(shortcode.into_bytes()),  CString::new(ident.into_bytes())) {
                        (Ok(shortcode_cstring), Ok(ident_cstring)) => {
                            ibus_lookup_table_append_candidate(table, ibus_text_new_from_string(shortcode_cstring.into_raw() as *mut gchar));
                            self.symbol_label_vec.push(ident_cstring);
                            if idx < (*table).page_size as usize {
                                ibus_lookup_table_set_label(table, idx as guint, ibus_text_new_from_static_string(self.symbol_label_vec.get_unchecked(idx).as_ptr()));
                            }
                        }
                        _ => {
                            log::error!("Failed string conversion for symbol lookup");
                        }
                    }
                }
                log::info!("{} candidates and {} labels", ibus_lookup_table_get_number_of_candidates(self.get_table()), (*(*self.get_table()).labels).len);
                ibus_engine_update_lookup_table(self.parent_engine_as_ibus_engine(), table, GBOOL_TRUE);
            },
            Err(err) => {
                log::error!("{}", err);
                return GBOOL_FALSE;
            }
        }

        GBOOL_TRUE
    }

    unsafe fn word_commit(&mut self) -> gboolean {
        if !self.lookup_visible || self.symbol_input {
            return GBOOL_FALSE;
        }

        let idx = ibus_lookup_table_get_cursor_in_page(self.get_table());
        let candidate = ibus_lookup_table_get_candidate(self.get_table(), idx);
        match CStr::from_ptr((*candidate).text as *const c_char).to_str() {
            Ok(word) => {
                match into_ibus_string(String::from(&word[self.word_buffer.len()..])) {
                    Ok(ibus_word) => {
                        ibus_engine_commit_text(self.parent_engine_as_ibus_engine(), ibus_word);
                    }
                    Err(err) => {
                        log::error!("Failed to convert slice back into ibus string: {}", err);
                    }
                }
            }
            Err(err) => {
                log::error!("Failed to convert word to string for commit: {}", err);
            }
        }

        self.word_buffer.clear();
        self.word_table_disable()
    }

    unsafe fn symbol_input_commit(&mut self) -> gboolean {
        if !self.symbol_input {
            log::error!("Symbol input commit called outside symbol input mode");
            return GBOOL_FALSE;
        }

        let idx = ibus_lookup_table_get_cursor_in_page(self.get_table());
        let symbol = ibus_lookup_table_get_label(self.get_table(), idx);
        ibus_engine_commit_text(self.parent_engine_as_ibus_engine(), symbol);

        self.symbol_input_disable()
    }
}



#[no_mangle]
pub unsafe extern "C" fn free_engine_core(engine_state: *mut EngineCore) {
    std::mem::drop(Box::from_raw(engine_state));
}

#[repr(C)]
pub struct WordPredictions {
    len: c_int,
    words: *mut *mut c_char
}

#[repr(C)]
pub struct SymbolPredictions {
    len: c_int,
    symbols: *mut *mut c_char,
    shortcodes: *mut *mut c_char
}

#[allow(unused_variables)]
#[no_mangle]
pub unsafe extern "C" fn ibus_eei_engine_process_key_event(engine: *mut IBusEngine, keyval: guint,
    keycode: guint, modifiers: guint) -> gboolean {

    log::info!("Process key {}", keyval);

    let engine_core = match EngineCore::get(engine) {
        Some(engine_ref) => engine_ref,
        None => {
            log::error!("Could not retrieve engine core");
            return GBOOL_FALSE
        }
    };


    if (modifiers & IBusModifierType_IBUS_CONTROL_MASK) == IBusModifierType_IBUS_CONTROL_MASK {
        //control key (and only control key) is held down
        return match keyval {
            IBUS_e => {
                engine_core.symbol_input_enable()
            }
            IBUS_w => {
                engine_core.word_table_enable()
            }
            _ => {
                GBOOL_FALSE
            }
        }
    } else if (modifiers & !IBusModifierType_IBUS_SHIFT_MASK) != 0 {
        return GBOOL_FALSE; //This also covers released keys with IBUS_RELEASE_MASK
    }

    match keyval {
        IBUS_space => {
            if engine_core.symbol_input {
                engine_core.symbol_input_disable();
            } else if engine_core.lookup_visible {
                engine_core.word_table_disable();
            }
            let ret = engine_core.commit_char(keyval);
            engine_core.word_buffer.clear();
            ret
        }
        IBUS_Return => {
            let ret = if engine_core.symbol_input {
                engine_core.symbol_input_commit()
            } else if engine_core.lookup_visible {
                engine_core.word_commit()
            } else {
                engine_core.commit_char(keyval)
            };
            engine_core.word_buffer.clear();
            ret
        }
        IBUS_Up => {
            if engine_core.lookup_visible {
                ibus_lookup_table_cursor_up(engine_core.get_table());
                engine_core.update_lookup_table();
                GBOOL_TRUE
            } else {
                GBOOL_FALSE
            }
        }
        IBUS_Down => {
            if engine_core.lookup_visible {
                ibus_lookup_table_cursor_down(engine_core.get_table());
                engine_core.update_lookup_table();
                GBOOL_TRUE
            } else {
                GBOOL_FALSE
            }
        }
        IBUS_BackSpace => {
            if engine_core.symbol_input {
                engine_core.symbol_preedit.pop();
                return engine_core.symbol_input_update();
            } else {
                if engine_core.lookup_visible {
                    engine_core.word_buffer.pop();
                    engine_core.word_table_update();
                }
            }
            GBOOL_FALSE
        }
        IBUS_Page_Down => {
            if engine_core.lookup_visible {
                log::info!("pageup");
                let res = ibus_lookup_table_page_down(engine_core.get_table());
                engine_core.update_lookup_table();
                return res
            }
            GBOOL_FALSE
        }
        IBUS_Page_Up => {
            if engine_core.lookup_visible {
                log::info!("pagedown");
                let res = ibus_lookup_table_page_up(engine_core.get_table());
                engine_core.update_lookup_table();
                return res
            }
            GBOOL_FALSE
        }
        IBUS_Escape => {
            if engine_core.symbol_input {
                return engine_core.symbol_input_disable();
            }
            GBOOL_FALSE
        }
        IBUS_space..=IBUS_asciitilde => {
            return if engine_core.symbol_input {
                engine_core.symbol_preedit.push((keyval as u8) as char);
                engine_core.symbol_input_update()
            } else {
                let ret = engine_core.commit_char(keyval);
                if engine_core.lookup_visible {
                    engine_core.word_table_update();
                }
                ret
            }
        }
        _ => GBOOL_FALSE
    }
}


#[no_mangle]
pub unsafe extern "C" fn configure_logging() {
    let logfile = FileAppender::builder()
        .encoder(Box::new(PatternEncoder::new("{l} - {m}\n")))
        .build("/home/josh/scrapbox/eei.log").unwrap();

    let config = Config::builder()
        .appender(Appender::builder().build("logfile", Box::new(logfile)))
        .build(Root::builder().appender("logfile").build(LevelFilter::Info)).unwrap();

    log4rs::init_config(config).unwrap();

    log::info!("Logging initialized");
}


