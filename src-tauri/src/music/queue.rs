use crate::db::types::Song;

pub struct Queue {
    songs: Vec<Song>,
    current_index: usize,
}

impl Queue {
    pub fn new() -> Self {
        Queue {
            songs: Vec::new(),
            current_index: 0,
        }
    }

    pub fn add_song(&mut self, song: Song) {
        self.songs.push(song);
    }

    pub fn remove_song(&mut self, index: usize) {
        if index < self.songs.len() {
            self.songs.remove(index);
            if self.current_index >= index && self.current_index > 0 {
                self.current_index -= 1;
            }
        }
    }

    pub fn next(&mut self) -> Option<&Song> {
        if self.songs.is_empty() {
            None
        } else {
            self.current_index = (self.current_index + 1) % self.songs.len();
            Some(&self.songs[self.current_index])
        }
    }

    pub fn prev(&mut self) -> Option<&Song> {
        if self.songs.is_empty() {
            None
        } else {
            self.current_index = (self.current_index + self.songs.len() - 1) % self.songs.len();
            Some(&self.songs[self.current_index])
        }
    }

    pub fn current(&self) -> Option<&Song> {
        if self.songs.is_empty() {
            None
        } else {
            Some(&self.songs[self.current_index])
        }
    }

    pub fn clear(&mut self) {
        self.songs.clear();
        self.current_index = 0;
    }
}
