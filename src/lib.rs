use std::{
    collections::LinkedList,
    fmt::Debug,
    io::{stdout, Error, ErrorKind, Result as IOResult, Write},
    thread,
    time::Duration,
};

use crossterm::{
    cursor::{Hide, MoveRight, MoveTo},
    event::{poll, read, KeyCode, KeyEvent, KeyEventKind, KeyModifiers},
    style::{PrintStyledContent, StyledContent, Stylize},
    terminal::{
        self, disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
        SetTitle,
    },
    ExecutableCommand, QueueableCommand,
};
use rand::Rng;

pub struct Game {
    /// 屏幕
    writer: Box<dyn Write>,
    /// 所有格子,二维方格
    cells: Vec<Vec<Cell>>,
    /// 蛇
    snack: Snack,
    /// 分数
    score: usize,
    speed: u64,
}
struct Snack {
    direction: Direction,
    head: (usize, usize),
    bodys: LinkedList<(usize, usize)>,
}
#[derive(PartialEq, Eq, Clone, Debug)]
pub struct Cell {
    /// 列
    x: usize,
    /// 行
    y: usize,
    changed_flag: bool,
    cell_type: CellType,
}
#[derive(PartialEq, Eq, Clone, Copy, Debug)]
enum CellType {
    Wall,
    SnackHead,
    SnackBody,
    Food,
    Empty,
}
#[derive(PartialEq, Eq, Clone, Copy, Debug)]
enum Direction {
    Left,
    Right,
    Up,
    Down,
}
impl Cell {
    /// 渲染
    fn render<W: Write>(&mut self, w: &mut W) -> IOResult<()> {
        w.queue(MoveTo(self.x as u16, self.y as u16))?
            .queue(PrintStyledContent(self.cell_style_content()))?;
        self.changed_flag = false;
        Ok(())
    }
    fn cell_style_content(&mut self) -> StyledContent<char> {
        match self.cell_type {
            CellType::Wall => '█'.blue().on_black(),
            CellType::SnackHead => '#'.green().on_black(),
            CellType::SnackBody => '#'.yellow().on_black(),
            CellType::Food => '$'.red().on_black().slow_blink(),
            CellType::Empty => '█'.black().on_black(),
        }
    }
    fn set_type(&mut self, t: CellType) {
        self.changed_flag = true;
        self.cell_type = t;
    }
}
impl Game {
    /// 创建初始化
    pub fn new() -> Result<Self, &'static str> {
        if let Ok((x, y)) = terminal::size() {
            if x < 60 || y < 20 {
                Err("窗口尺寸过小")
            } else {
                let mut cells = Vec::with_capacity(x as usize);
                for i in 0..x {
                    let mut columns = Vec::with_capacity(y as usize);
                    for j in 0..y {
                        columns.push(Cell {
                            x: i as usize,
                            y: j as usize,
                            changed_flag: false,
                            cell_type: CellType::Empty,
                        })
                    }
                    cells.push(columns);
                }
                let direction = Direction::Right;
                let snack = Snack {
                    direction,
                    head: (9, 7),
                    bodys: {
                        let mut bodys = LinkedList::new();
                        bodys.push_back((8, 7));
                        bodys.push_back((8, 8));
                        bodys.push_back((7, 8));
                        bodys.push_back((7, 9));
                        bodys.push_back((7, 10));
                        bodys.push_back((8, 10));
                        bodys.push_back((8, 11));
                        bodys
                    },
                };
                Ok(Game {
                    writer: Box::new(stdout()),
                    cells,
                    snack,
                    score: 0,
                    speed: 80,
                })
            }
        } else {
            Err("初始化失败:无法获取窗口尺寸")
        }
    }
    /// 构建场景,可以定义其他场景
    fn build_default(&mut self) -> IOResult<()> {
        self.writer
            .queue(EnterAlternateScreen)?
            .queue(SetTitle("Snack"))?
            .queue(Hide)?
            .flush()?;
        let width = self.cells.len();
        let height = self.cells[0].len();
        // wall
        self.cells.iter_mut().enumerate().for_each(|(x, column)| {
            column.iter_mut().enumerate().for_each(|(y, cell)| {
                if x == 0 || y == 0 || x == width - 1 || y == height - 1 {
                    cell.cell_type = CellType::Wall;
                }
            })
        });
        // snack
        let (head_x, head_y) = self.snack.head;
        self.cells[head_x][head_y].cell_type = CellType::SnackHead;
        self.snack
            .bodys
            .iter()
            .for_each(|&(x, y)| self.cells[x][y].cell_type = CellType::SnackBody);
        // food
        self.generage_food().unwrap();

        Ok(())
    }
    pub fn run(&mut self) -> IOResult<()> {
        //使用原始模式,这会禁用相关快捷键
        enable_raw_mode()?;
        //使用默认场景
        self.build_default()?;
        //渲染整个画面
        self.render_all()?;
        //title
        self.writer
            .queue(MoveTo((self.cells.len() as u16 - 6) / 2, 0))?
            .queue(PrintStyledContent("贪吃蛇".green().on_black()))?
            .flush()?;
        // score and tip
        self.print_score()?;
        self.poll()?; //开始游戏进程
        Ok(())
    }
    ///游戏循环
    fn poll(&mut self) -> IOResult<()> {
        thread::sleep(Duration::from_secs(2));
        let mut tmp_key = KeyEvent::new_with_kind(
            KeyCode::Char('d'),
            KeyModifiers::empty(),
            KeyEventKind::Release,
        );
        loop {
            if poll(Duration::from_millis(0))? {
                match read().unwrap() {
                    crossterm::event::Event::Key(key_event) => {
                        if key_event.kind == KeyEventKind::Release {
                            tmp_key = key_event;
                        }
                        // 按下escape会退出游戏循环
                        if let KeyCode::Esc = key_event.code {
                            self.writer.execute(LeaveAlternateScreen)?;
                            break;
                        }
                    }
                    _ => {}
                }
            } else {
                if let KeyCode::Char(c) = tmp_key.code {
                    match c {
                        'a' | 'A' => self.turn_around(Direction::Left),
                        's' | 'S' => self.turn_around(Direction::Down),
                        'd' | 'D' => self.turn_around(Direction::Right),
                        'w' | 'W' => self.turn_around(Direction::Up),
                        _ => {}
                    }
                }

                // 处理下一帧
                match self.collision_detection() {
                    (CellType::Wall, _) => {
                        thread::sleep(Duration::from_secs(2));
                        self.writer.execute(LeaveAlternateScreen).unwrap();
                        return Err(Error::new(ErrorKind::NotFound, "撞墙"));
                    }
                    (CellType::SnackHead, _) => {}
                    (CellType::SnackBody, _) => {
                        thread::sleep(Duration::from_secs(2));
                        self.writer.execute(LeaveAlternateScreen).unwrap();
                        return Err(Error::new(ErrorKind::NotFound, "自杀"));
                    }
                    (CellType::Food, (x, y)) => self.eat_food(x, y)?,
                    (CellType::Empty, (x, y)) => self.go(x, y),
                };
                // self.go();
                self.render_only_updated()?; //渲染更新的部分
                thread::sleep(Duration::from_millis(self.speed));
            }
        }
        Ok(())
    }
    pub fn score(&self) -> usize {
        self.score
    }
    /// 底部展示分数
    fn print_score(&mut self) -> IOResult<()> {
        self.writer
            .queue(MoveTo(4, self.cells[0].len() as u16 - 1))?
            .queue(PrintStyledContent("得分:".on_blue()))?
            .queue(PrintStyledContent(
                format!("{:^7?}", self.score).green().on_white().bold(),
            ))?
            .queue(MoveRight(4))?
            .queue(PrintStyledContent("Speed: ".on_blue()))?
            .queue(PrintStyledContent(self.speed.to_string().red().on_white()))?
            .queue(MoveRight(4))?
            .queue(PrintStyledContent("按Esc退出".grey().on_blue()))?
            .flush()?;

        Ok(())
    }
    ///设置转弯
    fn turn_around(&mut self, dir: Direction) {
        //只有拐弯的命令才会被处理
        if let (Direction::Left, Direction::Up)
        | (Direction::Left, Direction::Down)
        | (Direction::Right, Direction::Up)
        | (Direction::Right, Direction::Down)
        | (Direction::Up, Direction::Left)
        | (Direction::Up, Direction::Right)
        | (Direction::Down, Direction::Left)
        | (Direction::Down, Direction::Right) = (self.snack.direction, dir)
        {
            self.snack.direction = dir;
        }
    }
    ///正常走
    fn go(&mut self, x: usize, y: usize) {
        let (h_x, h_y) = self.snack.head;
        self.snack.bodys.push_front((h_x, h_y));
        self.cells[h_x][h_y].set_type(CellType::SnackBody);
        self.snack.head = (x, y);
        self.cells[x][y].set_type(CellType::SnackHead);
        //有可能没有body
        if let Some((x, y)) = self.snack.bodys.pop_back() {
            self.cells[x][y].set_type(CellType::Empty);
        };
    }
    ///碰撞检测
    fn collision_detection(&mut self) -> (CellType, (usize, usize)) {
        let (h_x, h_y) = self.snack.head;
        let (n_x, n_y) = match self.snack.direction {
            Direction::Left => (h_x as isize - 1, h_y as isize),
            Direction::Right => (h_x as isize + 1, h_y as isize),
            Direction::Up => (h_x as isize, h_y as isize - 1 as isize),
            Direction::Down => (h_x as isize, h_y as isize + 1),
        };
        if n_x < 0 || n_y < 0 {
            return (CellType::Wall, (0, 0));
        }
        (
            self.cells[n_x as usize][n_y as usize].cell_type,
            (n_x as usize, n_y as usize),
        )
    }
    ///随机生成食物
    fn generage_food(&mut self) -> IOResult<()> {
        let empty_cells = self
            .cells
            .iter()
            .flat_map(|column| {
                column.iter().filter_map(|c| {
                    if c.cell_type == CellType::Empty {
                        Some((c.x, c.y))
                    } else {
                        None
                    }
                })
            })
            .collect::<Vec<_>>();
        if empty_cells.is_empty() {
            self.writer.execute(LeaveAlternateScreen)?;
            Err(Error::new(ErrorKind::Other, "没有足够的空间生成食物"))
        } else {
            let index = rand::thread_rng().gen_range(0..empty_cells.len());
            let (x, y) = empty_cells[index];
            self.cells[x][y].set_type(CellType::Food);
            Ok(())
        }
    }
    ///吃
    fn eat_food(&mut self, x: usize, y: usize) -> IOResult<()> {
        self.generage_food()?;
        let (h_x, h_y) = self.snack.head;
        self.snack.bodys.push_front((h_x, h_y));
        self.cells[h_x][h_y].set_type(CellType::SnackBody);
        self.snack.head = (x, y);
        self.cells[x][y].set_type(CellType::SnackHead);
        self.score += 1;
        self.print_score()?;
        Ok(())
    }

    ///渲染全部格子
    fn render_all(&mut self) -> IOResult<()> {
        for c in self.cells.iter_mut().flat_map(|c| c) {
            c.render(&mut self.writer)?;
        }
        self.writer.flush()?;
        Ok(())
    }
    /// 只渲染需要更新的格子
    fn render_only_updated(&mut self) -> IOResult<()> {
        self.cells.iter_mut().for_each(|columns| {
            columns
                .iter_mut()
                .filter(|cell| cell.changed_flag)
                .for_each(|cell| {
                    cell.render(&mut self.writer).unwrap();
                })
        });
        self.writer.flush()?;
        Ok(())
    }
}
impl Drop for Game {
    fn drop(&mut self) {
        disable_raw_mode().unwrap();
    }
}
