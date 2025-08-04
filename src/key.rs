use core::panic;


pub struct KeyPool {
    keys: Vec<String>,
    current_idx: usize
}

impl KeyPool {

    pub fn new(kkeys: Vec<String>) -> KeyPool {
        if kkeys.is_empty() {
            panic!("Api key is empty")
        }
        KeyPool{
            keys: kkeys,
            current_idx: 0            
        }
    }

    pub fn get_key(&mut self) -> String {
        let keys: String = self.keys[self.current_idx].clone();
        self.current_idx = (self.current_idx+1) % self.keys.len();
        keys
    }

        
}
