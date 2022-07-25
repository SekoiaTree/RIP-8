use std::collections::HashMap;
use std::{env, mem};
use std::fs::File;
use std::io::Read;
use std::time::Duration;

use lazy_static::lazy_static;
use rodio::{OutputStream, OutputStreamHandle, Sink};
use rodio::source::SineWave;
use sdl2::event::Event;
use sdl2::keyboard::Keycode;
use sdl2::pixels::Color;

lazy_static! {
    static ref DEFAULT_MAPPINGS: HashMap<Keycode, usize> = [

    (Keycode::Num1, 0x1), (Keycode::Num2, 0x2), (Keycode::Num3, 0x3), (Keycode::Num4, 0xC),
    (Keycode::Q   , 0x4), (Keycode::W   , 0x5), (Keycode::E   , 0x6), (Keycode::R   , 0xD),
    (Keycode::A   , 0x7), (Keycode::S   , 0x8), (Keycode::D   , 0x9), (Keycode::F   , 0xE),
    (Keycode::Z   , 0xA), (Keycode::X   , 0x0), (Keycode::C   , 0xB), (Keycode::V   , 0xF),
    ].iter().copied().collect();
}

fn create_audio() -> Option<(OutputStream, OutputStreamHandle, Sink)> {
    let (_stream, handle) = OutputStream::try_default().ok()?;
    let sink = Sink::try_new(&handle).ok()?;
    let source = SineWave::new(1024.0);
    sink.set_volume(0.2);
    sink.pause();
    sink.append(source);
    Some((_stream, handle, sink))
}


fn main() {
    let sdl_context = sdl2::init().unwrap();
    let video_subsystem = sdl_context.video().unwrap();

    let window = video_subsystem.window("RIP-8", 1024, 512)
        .position_centered()
        .build()
        .expect("Failed to create window");

    let mut canvas = window.into_canvas().build().expect("Failed to create canvas");

    canvas.set_draw_color(Color::RGB(0, 0, 0));
    canvas.clear();
    canvas.present();

    let mut event_pump = sdl_context.event_pump().expect("Failed to create event pump");


    let audio = create_audio();

    let mut machine = Machine::new(audio.map(|(x, y, sink)| {
        mem::forget(x);
        mem::forget(y);
        sink
    }));

    let path = env::args().skip(1).next().expect("No input file.");
    let file = File::open(path).expect("Could not open file.");
    machine.load_program(file);
    'main: loop {
        machine.cycle();
        if machine.draw_flag {
            canvas.set_draw_color(Color::RGB(0, 0, 0));
            canvas.clear();
            for x in 0..64 {
                for y in 0..32 {
                    if machine.screen[y * 64 + x] {
                        canvas.set_draw_color(Color::RGB(255, 255, 255));
                        let rect = sdl2::rect::Rect::new((x * 16) as i32, (y * 16) as i32, 16, 16);
                        canvas.fill_rect(rect).expect("Failed to draw");
                    }
                }
            }
            canvas.present();

            machine.draw_flag = false;
            machine.draw_complete();
        }


        for event in event_pump.poll_iter() {
            match event {
                Event::Quit { .. } |
                Event::KeyDown { keycode: Some(Keycode::Escape), .. } => {
                    break 'main;
                }
                Event::KeyDown { keycode: Some(x), .. } => {
                    if let Some(key) = DEFAULT_MAPPINGS.get(&x) {
                        machine.key_pressed(*key);
                    }
                }
                Event::KeyUp { keycode: Some(x), .. } => {
                    if let Some(key) = DEFAULT_MAPPINGS.get(&x) {
                        machine.key_released(*key);
                    }
                }
                _ => {}
            }
        }

        std::thread::sleep(Duration::new(0, 1_000_000_000u32 / 240));
    }
}

const TIMER_DIVIDER : u8 = 4;
struct Machine {
    memory: [u8; 4096],
    stack: Vec<u16>,
    pc: usize,
    index_register: u16,
    registers: [u8; 16],
    keys: [bool; 16],
    screen: [bool; 64 * 32],
    delay_timer: u8,
    sound_timer: u8,
    frame_timer: u8,
    draw_flag: bool,
    audio: Option<Sink>,
    state: State,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
enum State {
    Running,
    Halted,
    WaitingForKey(usize),
}

type OpCode = u16;

impl Machine {
    const INSTRUCTIONS: [fn(&mut Self, OpCode); 16] = [
        Self::zero,
        Self::goto,
        Self::call,
        Self::cond_eq_const,
        Self::cond_neq_const,
        Self::cond_eq_reg,
        Self::set_const,
        Self::add_const,
        Self::arith,
        Self::cond_neq_reg,
        Self::set_index,
        Self::jump,
        Self::rand,
        Self::draw,
        Self::cond_key,
        Self::util,
    ];

    const ARITHMETIC: [fn(&mut Self, OpCode); 16] = [
        Self::set_reg,
        Self::bit_or,
        Self::bit_and,
        Self::bit_xor,
        Self::add_reg,
        Self::sub_reg,
        Self::shift_right,
        Self::rev_sub,
        Self::invalid_opcode_opcode,
        Self::invalid_opcode_opcode,
        Self::invalid_opcode_opcode,
        Self::invalid_opcode_opcode,
        Self::invalid_opcode_opcode,
        Self::invalid_opcode_opcode,
        Self::shift_left,
        Self::invalid_opcode_opcode
    ];

    const FONTSET: [u8; 0x50] = [
        0xF0, 0x90, 0x90, 0x90, 0xF0, // 0
        0x20, 0x60, 0x20, 0x20, 0x70, // 1
        0xF0, 0x10, 0xF0, 0x80, 0xF0, // 2
        0xF0, 0x10, 0xF0, 0x10, 0xF0, // 3
        0x90, 0x90, 0xF0, 0x10, 0x10, // 4
        0xF0, 0x80, 0xF0, 0x10, 0xF0, // 5
        0xF0, 0x80, 0xF0, 0x90, 0xF0, // 6
        0xF0, 0x10, 0x20, 0x40, 0x40, // 7
        0xF0, 0x90, 0xF0, 0x90, 0xF0, // 8
        0xF0, 0x90, 0xF0, 0x10, 0xF0, // 9
        0xF0, 0x90, 0xF0, 0x90, 0x90, // A
        0xE0, 0x90, 0xE0, 0x90, 0xE0, // B
        0xF0, 0x80, 0x80, 0x80, 0xF0, // C
        0xE0, 0x90, 0x90, 0x90, 0xE0, // D
        0xF0, 0x80, 0xF0, 0x80, 0xF0, // E
        0xF0, 0x80, 0xF0, 0x80, 0x80,  // F
    ];

    pub fn new(audio: Option<Sink>) -> Machine {
        let mut memory = [0; 4096];
        memory[..0x50].copy_from_slice(&Self::FONTSET);
        Machine {
            memory,
            stack: Vec::with_capacity(16),
            pc: 0x200,
            index_register: 0,
            registers: [0; 16],
            keys: [false; 16],
            screen: [false; 64 * 32],
            delay_timer: 0,
            sound_timer: 0,
            frame_timer: 0,
            draw_flag: false,
            audio,
            state: State::Running,
        }
    }

    pub fn load_program<R>(&mut self, mut program: R) where R: Read {
        program.read(&mut self.memory[0x200..]).expect("Could not read program.");
    }

    pub fn draw_complete(&mut self) {
        self.draw_flag = false;
    }

    pub fn key_pressed(&mut self, key: usize) {
        self.keys[key] = true;
    }

    pub fn key_released(&mut self, key: usize) {
        self.keys[key] = false;
    }

    pub fn fetch_opcode(&mut self) -> OpCode {
        let opcode = (self.memory[self.pc] as u16) << 8 | self.memory[self.pc + 1] as u16;
        self.pc += 2;
        opcode
    }

    pub fn cycle(&mut self) -> bool {
        match self.state {
            State::Running => {
                let opcode = self.fetch_opcode();
                let x = ((opcode & 0xF000) >> 12) as usize;

                Self::INSTRUCTIONS[x](self, opcode);
                if self.frame_timer == 0 {
                    self.delay_timer = self.delay_timer.saturating_sub(1);

                    if self.sound_timer == 1 {
                        if let Some(x) = &self.audio {
                            x.pause();
                        }
                    }
                    self.sound_timer = self.sound_timer.saturating_sub(1);
                    self.frame_timer = TIMER_DIVIDER;
                }
                self.frame_timer -= 1;

                if self.pc >= 0x1000 {
                    self.state = State::Halted;
                }
            }
            State::Halted => {}
            State::WaitingForKey(x) => {
                for (i, v) in self.keys.iter().enumerate() {
                    if *v {
                        self.registers[x] = i as u8;
                        self.state = State::Running;
                    }
                }
            }
        }
        self.state == State::Halted
    }

    fn invalid_opcode(&self, v: OpCode) {
        let mut stack_string = "[".to_string();
        for i in &self.stack {
            stack_string.push_str(&format!("0x{:04X}, ", i));
        }
        stack_string = stack_string.trim_end_matches(", ").to_string();
        stack_string.push_str("]");

        let mut register_string = "[".to_string();
        for i in &self.registers {
            register_string.push_str(&format!("0x{:02X}, ", i));
        }
        register_string = stack_string.trim_end_matches(", ").to_string();
        register_string.push_str("]");

        let builder : String = format!("\
        -----------CRASH INFO-----------\n\
        ERROR Unknown opcode: 0x{:04X}\n\
        --------------DATA--------------\n\
        Stack: {}\n\
        Current address: 0x{:04X}\n\
        Index selector: 0x{:04X}\n\
        Registers: {}\n\
        ---------END CRASH INFO---------", v, stack_string, self.pc, self.index_register, register_string);
        // TODO: full memory dump
        eprintln!("{}", builder);
        panic!();
    }

    fn invalid_opcode_opcode(&mut self, v: OpCode) {
        self.invalid_opcode(v);
    }

    fn zero(&mut self, v: OpCode) {
        match v & 0x00FF {
            0xE0 => {
                self.draw_flag = true;
                self.screen = [false; 64 * 32];
            }
            0xEE => self.pc = self.stack.pop().unwrap() as usize,
            _ => self.invalid_opcode(v),
        }
    }

    fn goto(&mut self, v: OpCode) {
        self.pc = (v & 0x0FFF) as usize;
    }

    fn call(&mut self, v: OpCode) {
        if self.stack.len() == self.stack.capacity() {
            panic!("Stack overflow! Stacktrace: {:?}", self.stack);
        }
        self.stack.push(self.pc as u16);
        self.pc = (v & 0x0FFF) as usize;
    }

    fn cond_eq_const(&mut self, v: OpCode) {
        let x = ((v & 0x0F00) >> 8) as usize;
        let n = (v & 0x00FF) as u8;
        if self.registers[x] == n {
            self.pc += 2;
        }
    }

    fn cond_neq_const(&mut self, v: OpCode) {
        let x = ((v & 0x0F00) >> 8) as usize;
        let n = (v & 0x00FF) as u8;
        if self.registers[x] != n {
            self.pc += 2;
        }
    }

    fn cond_eq_reg(&mut self, v: OpCode) {
        let x = ((v & 0x0F00) >> 8) as usize;
        let y = ((v & 0x00F0) >> 4) as usize;
        if self.registers[x] == self.registers[y] {
            self.pc += 2;
        }
    }

    fn set_const(&mut self, v: OpCode) {
        let x = ((v & 0x0F00) >> 8) as usize;
        let n = (v & 0x00FF) as u8;
        self.registers[x] = n;
    }

    fn add_const(&mut self, v: OpCode) {
        let x = ((v & 0x0F00) >> 8) as usize;
        let n = (v & 0x00FF) as u8;
        self.registers[x] = self.registers[x].wrapping_add(n);
    }

    fn arith(&mut self, v: OpCode) {
        Self::ARITHMETIC[(v & 0x000F) as usize](self, v);
    }

    fn set_reg(&mut self, v: OpCode) {
        let x = ((v & 0x0F00) >> 8) as usize;
        let y = ((v & 0x00F0) >> 4) as usize;
        self.registers[x] = self.registers[y];
    }

    fn bit_or(&mut self, v: OpCode) {
        let x = ((v & 0x0F00) >> 8) as usize;
        let y = ((v & 0x00F0) >> 4) as usize;
        self.registers[x] = self.registers[x] | self.registers[y];
    }

    fn bit_and(&mut self, v: OpCode) {
        let x = ((v & 0x0F00) >> 8) as usize;
        let y = ((v & 0x00F0) >> 4) as usize;
        self.registers[x] = self.registers[x] & self.registers[y];
    }

    fn bit_xor(&mut self, v: OpCode) {
        let x = ((v & 0x0F00) >> 8) as usize;
        let y = ((v & 0x00F0) >> 4) as usize;
        self.registers[x] = self.registers[x] ^ self.registers[y];
    }

    fn add_reg(&mut self, v: OpCode) {
        let x = ((v & 0x0F00) >> 8) as usize;
        let y = ((v & 0x00F0) >> 4) as usize;

        let (result, overflow) = self.registers[x].overflowing_add(self.registers[y]);
        self.registers[x] = result;
        self.registers[0xF] = overflow as u8;
    }

    fn sub_reg(&mut self, v: OpCode) {
        let x = ((v & 0x0F00) >> 8) as usize;
        let y = ((v & 0x00F0) >> 4) as usize;

        let (result, overflow) = self.registers[x].overflowing_sub(self.registers[y]);
        self.registers[x] = result;
        self.registers[0xF] = overflow as u8;
    }

    fn shift_right(&mut self, v: OpCode) {
        let x = ((v & 0x0F00) >> 8) as usize;
        self.registers[0xF] = self.registers[x] & 0x1;
        self.registers[x] >>= 1;
    }

    fn shift_left(&mut self, v: OpCode) {
        let x = ((v & 0x0F00) >> 8) as usize;
        self.registers[0xF] = self.registers[x] & 0x80;
        self.registers[x] <<= 1;
    }

    fn rev_sub(&mut self, v: OpCode) {
        let x = ((v & 0x0F00) >> 8) as usize;
        let y = ((v & 0x00F0) >> 4) as usize;

        let (result, overflow) = self.registers[y].overflowing_sub(self.registers[x]);
        self.registers[x] = result;
        self.registers[0xF] = overflow as u8;
    }

    fn cond_neq_reg(&mut self, v: OpCode) {
        let x = ((v & 0x0F00) >> 8) as usize;
        let y = ((v & 0x00F0) >> 4) as usize;
        if self.registers[x] != self.registers[y] {
            self.pc += 2;
        }
    }

    fn set_index(&mut self, v: OpCode) {
        self.index_register = v & 0x0FFF;
    }

    fn jump(&mut self, v: OpCode) {
        self.pc = (v & 0x0FFF) as usize + self.registers[0x0] as usize;
    }

    fn rand(&mut self, v: OpCode) {
        let x = ((v & 0x0F00) >> 8) as usize;
        let n = (v & 0x00FF) as u8;
        self.registers[x] = rand::random::<u8>() & n;
    }

    fn draw(&mut self, v: OpCode) {
        let x = ((v & 0x0F00) >> 8) as usize;
        let y = ((v & 0x00F0) >> 4) as usize;
        let base_y = self.registers[y] as usize;
        let base_x = self.registers[x] as usize;
        let n = (v & 0x000F) as usize;
        self.registers[0xF] = 0;
        for i in 0..n {
            if self.index_register as usize + i >= self.memory.len() {
                break;
            }
            let sprite = self.memory[self.index_register as usize + i];
            let sprite_y = base_y + i;
            for j in 0..8 {
                let sprite_x = base_x + j;
                let pixel = (sprite >> (7 - j)) & 0x1;
                let pixel_index = sprite_y * 64 + sprite_x;
                if pixel_index >= self.screen.len() {
                    continue;
                }
                if pixel == 1 && self.screen[pixel_index] {
                    self.registers[0xF] = 1;
                }
                self.screen[pixel_index] = (pixel == 1) != self.screen[pixel_index];
            }
        }

        self.draw_flag = true;
    }

    fn cond_key(&mut self, v: OpCode) {
        if v & 0x00FF == 0x009E {
            self.cond_key_pressed(v);
        } else {
            self.cond_key_not_pressed(v);
        }
    }

    fn cond_key_pressed(&mut self, v: OpCode) {
        let x = ((v & 0x0F00) >> 8) as usize;
        let key = self.registers[x] as usize;
        if self.keys[key] {
            self.pc += 2;
        }
    }

    fn cond_key_not_pressed(&mut self, v: OpCode) {
        let x = ((v & 0x0F00) >> 8) as usize;
        let key = self.registers[x] as usize;
        if !self.keys[key] {
            self.pc += 2;
        }
    }

    fn util(&mut self, v: OpCode) {
        match v & 0x00FF {
            0x07 => self.get_delay(v),
            0x0A => self.await_key(v),
            0x15 => self.set_delay(v),
            0x18 => self.set_sound(v),
            0x1E => self.add_index(v),
            0x29 => self.set_index_char(v),
            0x33 => self.set_index_bcd(v),
            0x55 => self.reg_dump(v),
            0x65 => self.reg_load(v),
            _ => self.invalid_opcode(v)
        }
    }

    fn get_delay(&mut self, v: OpCode) {
        let x = ((v & 0x0F00) >> 8) as usize;
        self.registers[x] = self.delay_timer;
    }

    fn await_key(&mut self, v: OpCode) {
        for i in 0..16 {
            if self.keys[i] {
                let x = ((v & 0x0F00) >> 8) as usize;
                self.registers[x] = i as u8;
                return;
            }
        }
        self.state = State::WaitingForKey(((v & 0x0F00) >> 8) as usize);
    }

    fn set_delay(&mut self, v: OpCode) {
        let x = ((v & 0x0F00) >> 8) as usize;
        self.delay_timer = self.registers[x];
    }

    fn set_sound(&mut self, v: OpCode) {
        let x = ((v & 0x0F00) >> 8) as usize;
        self.sound_timer = self.registers[x];
        if let Some(x) = &mut self.audio {
            x.play();
        }
    }

    fn add_index(&mut self, v: OpCode) {
        self.index_register += v & 0x0FFF;
        self.registers[0xF] = if self.index_register > 0xFFF { 1 } else { 0 };
    }

    fn set_index_char(&mut self, v: OpCode) {
        let x = ((v & 0x0F00) >> 8) as usize;
        self.index_register = self.registers[x] as u16 * 5;
    }

    fn set_index_bcd(&mut self, v: OpCode) {
        let x = ((v & 0x0F00) >> 8) as usize;
        let bcd = self.registers[x];
        self.memory[self.index_register as usize] = bcd / 100;
        self.memory[self.index_register as usize + 1] = (bcd / 10) % 10;
        self.memory[self.index_register as usize + 2] = bcd % 10;
    }

    fn reg_dump(&mut self, v: OpCode) {
        let x = ((v & 0x0F00) >> 8) as usize;
        for i in 0..=x {
            self.memory[self.index_register as usize + i] = self.registers[i];
        }
        self.index_register += x as u16 + 1;
    }

    fn reg_load(&mut self, v: OpCode) {
        let x = ((v & 0x0F00) >> 8) as usize;
        for i in 0..=x {
            self.registers[i] = self.memory[self.index_register as usize + i];
        }
        self.index_register += x as u16 + 1;
    }
}