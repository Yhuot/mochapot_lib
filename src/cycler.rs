use std::{sync::{RwLock, RwLockReadGuard}};

pub enum CyclerErrors {
    DuplicatedOption,
    OptionNotFound,
    EmptyCycler,
    AttemptedLastOptionRemoval,
    PossibleDataCorruption
}

pub struct MochaCycler<T> where T: Copy {
    index: RwLock<u32>,
    options: RwLock<Vec<T>>
}

impl<T> MochaCycler<T> where T: Copy + PartialEq{
    pub fn new(options: Vec<T>) -> Result<MochaCycler<T>, CyclerErrors> {
        let option_count = options.len() as u32;
        if option_count < 1 {
            return Err(CyclerErrors::EmptyCycler)
        }
        Ok(MochaCycler { index: RwLock::new(0), options: RwLock::new(options) })
    }

    pub fn add_option(&self, new_option: T) -> Result<u32, CyclerErrors> {
        let index: u32;
        {
            let mut writer = self.options.write().unwrap();

            if writer.contains(&new_option) {
                return Err(CyclerErrors::DuplicatedOption)
            }

            index = writer.len() as u32;

            writer.push(new_option);

        }

        self.adjust_index();

        Ok(index)
        
    }

    fn add_option2(&self, new_option: T) -> Result<u32, CyclerErrors> {

        let mut writer = self.options.write().unwrap();

        if writer.contains(&new_option) {
            return Err(CyclerErrors::DuplicatedOption)
        }

        let index = writer.len() as u32;

        writer.push(new_option);

        Ok(index)
        
    }

    pub fn add_options(&self, new_options: Vec<T>) -> Vec<Result<u32, CyclerErrors>> {
        let mut results = Vec::new();
        for i in new_options {
            results.push(self.add_option2(i));
        }

        self.adjust_index();
        return results
    }

    pub fn remove_option_by_index(&self, index: u32) -> Result<T, CyclerErrors> {
        let result: Result<T, CyclerErrors>;
        {
            let mut writer = self.options.write().unwrap();
            if writer.len() as u32 <= index {
                return Err(CyclerErrors::OptionNotFound)
            }
            result = Ok(writer.remove(index as usize))
        }
        self.adjust_index();
        result
    }

    pub fn remove_option(&self, option: T) -> Result<T, CyclerErrors> {
        let result: Result<T, CyclerErrors>;
        {
            let mut writer = self.options.write().unwrap();
            result = match writer.iter().position(|value| value == &option) {
                Some(index) => {
                    Ok(writer.remove(index as usize))
                },
                None => Err(CyclerErrors::OptionNotFound),
            };
        }
        self.adjust_index();
        result
    }

    fn remove_option_by_index2(&self, index: u32) -> Result<T, CyclerErrors> {
        let mut writer = self.options.write().unwrap();
        if writer.len() as u32 <= index {
            return Err(CyclerErrors::OptionNotFound)
        }
        Ok(writer.remove(index as usize))
    }

    fn remove_option2(&self, option: T) -> Result<T, CyclerErrors> {
        let mut writer = self.options.write().unwrap();
        match writer.iter().position(|value| value == &option) {
            Some(index) => {
                Ok(writer.remove(index as usize))
            },
            None => Err(CyclerErrors::OptionNotFound),
        }
    }

    pub fn remove_options_by_index(&self, indexes: Vec<u32>) -> Vec<Result<T, CyclerErrors>> {
        let mut results = Vec::new();
        for i in indexes {
            results.push(self.remove_option_by_index2(i));
        }

        self.adjust_index();
        return results
    }

    pub fn remove_options(&self, indexes: Vec<T>) -> Vec<Result<T, CyclerErrors>> {
        let mut results = Vec::new();
        for i in indexes {
            results.push(self.remove_option2(i));
        }

        self.adjust_index();
        return results
    }

    // yes, i am paranoid
    fn adjust_index(&self) {
        let mut index_writer = self.index.write().unwrap();
        let options_reader = self.options.read().unwrap();
        let option_count = options_reader.len() as u32;
        if *index_writer >= option_count {
            *index_writer = *index_writer % option_count;
        }
    }

    pub fn get_current(&self) -> T {
        let index_reader = self.index.read().unwrap();
        let options_reader = self.options.read().unwrap();
        options_reader[*index_reader as usize]
    }

    pub fn get_next(&self, step: u32) -> T{
        let index_reader = self.index.read().unwrap();
        let options_reader = self.options.read().unwrap();
        let option_count = options_reader.len() as u32;
        let target = (*index_reader + step) % option_count;
        
        options_reader[target as usize]
    }

    pub fn get_previous(&self, step: u32) -> T{
        let option_reader = self.options.read().unwrap();
        self.get_next(option_reader.len() as u32 - (step % option_reader.len() as u32))
    }

    pub fn advance(&self, count: u32) {
        let mut write_lock = self.index.write().unwrap();
        let option_reader = self.options.read().unwrap();
        *write_lock = (*write_lock + count) % option_reader.len() as u32;
    }

    fn advance2(&self, count: u32, option_reader: &RwLockReadGuard<'_, Vec<T>>) {
        let mut write_lock = self.index.write().unwrap();
        *write_lock = (*write_lock + count) % option_reader.len() as u32;
    }

    pub fn roll_back(&self, count: u32) {
        let option_reader = self.options.read().unwrap();
        self.advance2(option_reader.len() as u32 - (count % option_reader.len() as u32), &option_reader);
    }

    pub fn advance_get(&self, count: u32) -> T{
        self.advance(count);
        self.get_current()
    }

    pub fn roll_back_get(&self, count: u32) -> T{
        self.roll_back(count);
        self.get_current()
    }

    pub fn get_advance(&self, count: u32) -> T{
        let e = self.get_current();
        self.advance(count);
        e
    }

    pub fn get_roll_back(&self, count: u32) -> T{
        let e = self.get_current();
        self.roll_back(count);
        e
    }

}
