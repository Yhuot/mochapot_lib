use std::{sync::{RwLock, RwLockReadGuard, RwLockWriteGuard}};

pub enum CyclerErrors {
    DuplicatedOption,
    OptionNotFound,
    EmptyCycler,
    AttemptedLastOptionRemoval,
    PossibleDataCorruption
}

pub struct MochaCycler<T> where T: Copy {
    index: RwLock<usize>,
    options: RwLock<Vec<T>>
}

impl<T> MochaCycler<T> where T: Copy + PartialEq{
    pub fn new(options: Vec<T>) -> Result<MochaCycler<T>, CyclerErrors> {
        let option_count = options.len();
        if option_count < 1 {
            return Err(CyclerErrors::EmptyCycler)
        }
        Ok(MochaCycler { index: RwLock::new(0), options: RwLock::new(options) })
    }

    pub fn add_option(&self, new_option: T) -> Result<usize, CyclerErrors> {
        let mut writer = self.options.write().unwrap();

        let result = self.add_option_inner(new_option, &mut writer);

        self.adjust_index();

        return result;
    }

    fn add_option_inner(&self, new_option: T, writer: &mut RwLockWriteGuard<'_, Vec<T>>) -> Result<usize, CyclerErrors> {

        if writer.contains(&new_option) {
            return Err(CyclerErrors::DuplicatedOption)
        }

        let index = writer.len();

        writer.push(new_option);

        Ok(index)
        
    }

    pub fn add_options(&self, new_options: Vec<T>) -> Vec<Result<usize, CyclerErrors>> {
        let mut results = Vec::new();
        let mut writer = self.options.write().unwrap();
        for i in new_options {
            results.push(self.add_option_inner(i, &mut writer));
        }

        self.adjust_index();
        return results
    }

    pub fn remove_option_by_index(&self, index: usize) -> Result<T, CyclerErrors> {
        let result: Result<T, CyclerErrors>;
        {
            let mut writer = self.options.write().unwrap();
            result = self.remove_option_by_index_inner(index, &mut writer);
        }
        self.adjust_index();
        result
    }

    fn remove_option_by_index_inner(&self, index: usize, writer: &mut RwLockWriteGuard<'_, Vec<T>>) -> Result<T, CyclerErrors> {
        if writer.len() < 2{
            return Err(CyclerErrors::AttemptedLastOptionRemoval)
        }
        if writer.len() <= index {
            return Err(CyclerErrors::OptionNotFound)
        }
        Ok(writer.remove(index as usize))
    }

    pub fn remove_option(&self, option: T) -> Result<T, CyclerErrors> {
        let result: Result<T, CyclerErrors>;
        {
            let mut writer = self.options.write().unwrap();
            result = self.remove_option_inner(option, &mut writer);
        }
        self.adjust_index();
        result
    }

    fn remove_option_inner(&self, option: T, writer: &mut RwLockWriteGuard<'_, Vec<T>>) -> Result<T, CyclerErrors> {
        if writer.len() < 2{
            return Err(CyclerErrors::AttemptedLastOptionRemoval)
        }
        let result = match writer.iter().position(|value| value == &option) {
            Some(index) => {
                Ok(writer.remove(index as usize))
            },
            None => Err(CyclerErrors::OptionNotFound),
        };
        result
    }

    pub fn remove_options_by_index(&self, mut indexes: Vec<usize>) -> Vec<Result<T, CyclerErrors>> {
        // let us not have index mismatch fuckery, shall we?
        indexes.sort();
        indexes.reverse();
        let mut results = Vec::new();
        {
            let mut writer = self.options.write().unwrap();
            for i in indexes {
                results.push(self.remove_option_by_index_inner(i, &mut writer));
            }
        }
        self.adjust_index();
        return results
    }

    pub fn remove_options(&self, options: Vec<T>) -> Vec<Result<T, CyclerErrors>> {
        let mut results = Vec::new();
        {
            let mut writer = self.options.write().unwrap();
            for i in options {
                results.push(self.remove_option_inner(i, &mut writer));
            }
        }
        self.adjust_index();
        return results
    }

    // yes, i am paranoid
    fn adjust_index(&self) {
        let options_reader = self.options.read().unwrap();
        let mut index_writer = self.index.write().unwrap();
        let option_count = options_reader.len();
        if *index_writer >= option_count {
            *index_writer = *index_writer % option_count;
        }
    }

    pub fn get_current(&self) -> T {
        let options_reader = self.options.read().unwrap();
        let index_reader = self.index.read().unwrap();
        options_reader[*index_reader as usize]
    }

    pub fn get_next(&self, step: usize) -> T{
        let options_reader = self.options.read().unwrap();
        self.get_next_inner(step, &options_reader)
    }

    pub fn get_next_inner(&self, step: usize, options_reader: &RwLockReadGuard<'_, Vec<T>>) -> T{
        let index_reader = self.index.read().unwrap();
        let option_count = options_reader.len();
        let target = (*index_reader + step) % option_count;
        
        options_reader[target as usize]
    }

    pub fn get_previous(&self, step: usize) -> T{
        let option_reader = self.options.read().unwrap();
        let option_count = option_reader.len();
        let step_back = option_count - (step % option_count);
        self.get_next_inner(step_back, &options_reader)
    }

    pub fn advance(&self, count: usize) {
        let option_reader = self.options.read().unwrap();
        self.advance_inner(count, &option_reader)
    }

    fn advance_inner(&self, count: usize, option_reader: &RwLockReadGuard<'_, Vec<T>>) {
        let mut write_lock = self.index.write().unwrap();
        *write_lock = (*write_lock + count) % option_reader.len();
    }

    pub fn roll_back(&self, count: usize) {
        let option_reader = self.options.read().unwrap();
        self.advance_inner(option_reader.len() - (count % option_reader.len()), &option_reader);
    }

    pub fn advance_get(&self, count: usize) -> T{
        self.advance(count);
        self.get_current()
    }

    pub fn roll_back_get(&self, count: usize) -> T{
        self.roll_back(count);
        self.get_current()
    }

    pub fn get_advance(&self, count: usize) -> T{
        let e = self.get_current();
        self.advance(count);
        e
    }

    pub fn get_roll_back(&self, count: usize) -> T{
        let e = self.get_current();
        self.roll_back(count);
        e
    }

}
