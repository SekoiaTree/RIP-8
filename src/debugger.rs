use sdl2::gfx::primitives::DrawRenderer;
use sdl2::pixels::Color;
use sdl2::render::Canvas;
use sdl2::video::Window;
use crate::Machine;

pub(crate) struct Debugger {
    active : bool,
    divider : u8,
    counter : u8,
    machine : Machine,
    remaining_steps : u8
}

impl Debugger {
    pub fn new(machine : Machine) -> Self {
        Debugger {
            active : false,
            divider : 1,
            counter: 0,
            remaining_steps: 0,
            machine
        }
    }

    pub fn toggle_pause(&mut self) {
        self.active = !self.active;
    }

    pub(crate) fn key_pressed(&mut self, key: usize) {
        if self.active {
            self.machine.key_pressed(key);
        }
    }

    pub(crate) fn key_released(&mut self, key: usize) {
        if self.active {
            self.machine.key_released(key);
        }
    }

    pub fn step(&mut self) {
        self.remaining_steps += 1;
    }

    pub fn cycle(&mut self, canvas : &mut Canvas<Window>, dbg_canvas : Option<&mut Canvas<Window>>) {
        if self.active {
            self.counter += 1;
            if self.counter == self.divider {
                self.counter = 0;
                self.machine_cycle(canvas, dbg_canvas);
            }
        } else if self.remaining_steps > 0 {
            self.remaining_steps -= 1;
            self.machine_cycle(canvas, dbg_canvas);
        }
    }

    fn machine_cycle(&mut self, canvas : &mut Canvas<Window>, dbg_canvas : Option<&mut Canvas<Window>>) {
        self.machine.cycle();

        if self.machine.draw_flag {
            canvas.set_draw_color(Color::RGB(0, 0, 0));
            canvas.clear();
            for x in 0..64 {
                for y in 0..32 {
                    if self.machine.screen[y * 64 + x] {
                        canvas.set_draw_color(Color::RGB(255, 255, 255));
                        let rect = sdl2::rect::Rect::new((x * 16) as i32, (y * 16) as i32, 16, 16);
                        canvas.fill_rect(rect).expect("Failed to draw");
                    }
                }
            }
            canvas.present();

            self.machine.draw_flag = false;
            self.machine.draw_complete();
        }


        if let Some(dbg_canvas) = dbg_canvas {
            dbg_canvas.set_draw_color(Color::RGB(0, 0, 0));
            dbg_canvas.clear();
            let data_x = "REGISTERS".len() as i16 * 8 + 16;

            let draw_string = |x: i16, y: i16, text: String| {
                for (i, c) in text.chars().enumerate() {
                    dbg_canvas.character(x + i as i16 * 8, y, c, Color::RGB(255, 255, 255)).expect("Failed to draw");
                }
            };

            draw_string(0, 10, "REGISTERS".to_string());

            for (i, x) in self.machine.registers.iter().enumerate() {
                draw_string(data_x+i as i16*32, 10, format!("{:02X}", x));
            }

            draw_string(0, 20, "PC/OPCODE".to_string());
            draw_string(data_x, 20, format!("0x{:04X}  0x{:04X}", self.machine.pc, (self.machine.memory[self.machine.pc] as u16) << 8 | self.machine.memory[self.machine.pc + 1] as u16));

            draw_string(0, 30, "STACK".to_string());
            for (i, x) in (&self.machine.stack).iter().enumerate() {
                draw_string(data_x+i as i16*64, 30, format!("0x{:04X}, ", x));
            }

            draw_string(0, 40, "INDEX REG".to_string());
            draw_string(data_x, 40, format!("0x{:04X}", self.machine.index_register));

            draw_string(20*8, 40, "DELAY".to_string());
            draw_string(data_x+20*8, 40, format!("0x{:02X}", self.machine.delay_timer));

            draw_string(40*8, 40, "SOUND".to_string());
            draw_string(data_x+40*8, 40, format!("0x{:02X}", self.machine.sound_timer));
            dbg_canvas.present();
        }
    }
}