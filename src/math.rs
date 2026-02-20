use std::cell::{RefCell};
use std::collections::HashSet;
use std::ops::{Deref};
use std::rc::Weak;
use rusqlite::{Connection, Result, Error, Row};
use std::rc::Rc;

pub enum ParsingState {
    Initial,
    FoundDigit,
    Accepted
}
// An identifier for a network component
#[derive(PartialEq, Debug, Copy, Clone)]
#[derive(Eq, Hash)]
pub struct Identifier {
    // A directed connection between two junctions
    pub link:u16,
    // A possibly road segments such as straights or circular curves
    pub tile:u16,
    // An individual piece of road
    pub segment:u16,
    // A label for a lateral portion of a segment
    pub lane:i16
}

impl Identifier {
    pub fn new(link:u16,tile:u16,segment:u16,lane:i16) -> Identifier {
        Identifier {
            link, tile, segment, lane
        }
    }

    pub fn parse(str:&str) -> Result<Identifier, &str> {
        let mut link:u16 = 0;
        let mut tile:u16 = 0;
        let mut segment:u16 = 0;
        let mut lane:i16 = 0;
        let mut state : ParsingState = ParsingState::Initial;
        let mut digits:&str;
        let mut digits_start = 0;
        let mut digits_end = 0;
        let mut i = 0;
        let mut allow_negative = false;
        let mut index = 0;
        for c in str.chars() {
            match state {
                ParsingState::Initial => {
                    if c.is_digit(10) || (c == '-' && allow_negative) {
                        digits_start = index;
                        digits_end = index+1;
                        state = ParsingState::FoundDigit;
                    }
                    else if c == '-' {
                        return Err("Expected whole number, got minus sign");
                    }
                },
                ParsingState::FoundDigit => {
                    if c.is_digit(10) {
                        digits_end += 1;
                    }
                    else if c == '.' {
                        digits = &str[digits_start..digits_end];
                        if i<4 {
                            if i==0 {
                                link = digits.parse::<u16>().unwrap_or(0);
                            }
                            else if i==1 {
                                tile = digits.parse::<u16>().unwrap_or(0);
                            }
                            else if i==2 {
                                segment = digits.parse::<u16>().unwrap_or(0);
                            }
                            else if i==3 {
                                lane = digits.parse::<i16>().unwrap_or(0);
                            }
                            i+=1;
                            if i == 3 {
                                allow_negative = true;
                            }
                            digits_start = 0;
                            digits_end = 0;
                            state = ParsingState::Initial;
                        }
                        else {
                            state = ParsingState::Accepted;
                        }
                    }
                },
                ParsingState::Accepted => {
                    break;
                }
            }
            index+=1;
        }
        if let ParsingState::FoundDigit = state && i==3 {
            digits = &str[digits_start..digits_end];
            lane = digits.parse::<i16>().unwrap();
        }
        Ok(Identifier {
            link,
            tile,
            segment,
            lane,
        })
    }
}

// An indication of which fields of an Identifier are relevant for a query
#[derive(PartialEq,Debug,Copy,Clone)]
#[derive(Eq, Hash)]
pub struct Mask {
    pub link:bool,
    pub tile:bool,
    pub segment:bool,
    pub lane:bool
}

impl Mask {
    pub fn new(link:bool,tile:bool,segment:bool,lane:bool) -> Mask {
        Mask {
            link, tile, segment, lane
        }
    }

    pub fn parse(str:&str) -> Mask {
        let mut state : ParsingState = ParsingState::Initial;
        let mut flags = [true,true,true,true];
        let mut i = 0;
        for c in str.chars() {
            match state {
                ParsingState::Initial => {
                    if c.is_digit(10) {
                        if i<flags.len() {
                            if c.to_digit(10).unwrap() != 0 {
                                flags[i] = true;
                            }
                            else {
                                flags[i] = false;
                            }
                            state = ParsingState::FoundDigit;
                            i+=1;
                        }
                        else {
                            state = ParsingState::Accepted;
                        }
                    }
                },
                ParsingState::FoundDigit => {
                    if c == '.' {
                        state = ParsingState::Initial;
                    }
                },
                ParsingState::Accepted => {
                    break;
                }
            }
        }
        Mask {
            link:flags[0],
            tile:flags[1],
            segment:flags[2],
            lane:flags[3]
        }
    }
}


#[derive(PartialEq, Debug, Copy, Clone)]
#[derive(Eq, Hash)]
pub struct LogicalAddress {
    id : Identifier,
    mask : Mask,
}

impl LogicalAddress {
    pub fn new(id:Identifier, mask:Mask) -> LogicalAddress {
        LogicalAddress {
            id,
            mask
        }
    }

    pub fn parse(id:&str) -> Result<LogicalAddress,&str> {
        let mut iter = id.split('/').enumerate();
        let id = iter.next().unwrap_or((0,"")).1;
        if id == "" {
            return Err("Expected some content before the '/'");
        }
        let mask = iter.next().unwrap_or((0,"1.1.1.1")).1;
        let id = Identifier::parse(id);
        let id = match id {
            Ok(ok) => {
                ok
            }
            Err(msg) => return Err(msg)
        };
        let mask = Mask::parse(mask);
        Ok(LogicalAddress {
            id,
            mask
        })
    }
}

// A high-level description of a place on the road network
struct Place {
    name: String,
    offset: f64,
    distance: f64,
    loft: f64,
}

pub struct InertialCoord {
    pub x: f64,
    pub y: f64,
    pub z: f64
}

pub struct LogicalCoord {
    pub addr: LogicalAddress,
    pub offset: f64,
    pub distance: f64,
    pub loft:f64
}

impl InertialCoord {
    pub fn new(x: f64, y: f64, z: f64) -> InertialCoord {
        InertialCoord {
            x,y,z
        }
    }
}
impl LogicalCoord {
    pub fn new(addr: LogicalAddress, offset: f64, distance: f64, loft: f64) -> LogicalCoord {
        LogicalCoord {
            addr,
            offset,
            distance,
            loft
        }
    }

    pub fn empty() -> LogicalCoord {
        LogicalCoord {
            addr:LogicalAddress::new(Identifier::new(0,0,0,0), Mask::new(false,false,false,false)),
            offset:0.0,
            distance:0.0,
            loft:0.0
        }
    }
}

// Currently an infinite straight
pub struct Curve {
    points : Vec<InertialCoord>,
}

impl Curve {
    pub fn new() -> Curve {
        Curve {
            points: Vec::new(),
        }
    }

    pub fn logical_to_inertial(&self, logical: &LogicalCoord, inertial: &mut InertialCoord) {
        inertial.x = logical.offset;
        inertial.y = logical.distance;
        inertial.z = logical.loft;
    }

    pub fn inertial_to_logical(&self, inertial: &InertialCoord, logical: &mut LogicalCoord) {
        logical.offset = inertial.x;
        logical.distance = inertial.y;
        logical.loft = inertial.z;
    }
}

pub enum SegmentType {
    Unknown,
    Straight
}
pub struct Segment {
    tile:u16,
    x:f64,
    y:f64,
    z:f64,
    h:f64,
    p:f64,
    r:f64,
    segment_type:SegmentType
}

impl Segment {
    pub fn new() -> Segment {
        Segment {
            tile:0,
            x:0.0,
            y:0.0,
            z:0.0,
            h:0.0,
            p:0.0,
            r:0.0,
            segment_type:SegmentType::Straight
        }
    }

    pub fn from_query(row:&Row) -> Segment {
        Segment {
            tile:row.get("tile_id").unwrap(),
            x:row.get("x").unwrap(),
            y:row.get("y").unwrap(),
            z:row.get("z").unwrap(),
            h:row.get("h").unwrap(),
            p:row.get("p").unwrap(),
            r:row.get("r").unwrap(),
            segment_type:Segment::segment_type_from_field(row.get("type").unwrap())
        }
    }

    pub fn segment_type_from_field(field:i32) -> SegmentType {
        if field == 0 {
            return SegmentType::Straight
        }
        SegmentType::Unknown
    }
}
pub struct Tile {
    id:u16,
    link:u16,
    segments: Vec<Box<Segment>>
}

impl Tile {
    fn from_query(id: u16, link:u16) -> Tile {
        Tile {
            id,
            link,
            segments: Vec::new()
        }
    }

}

#[derive(Copy,Clone)]
pub struct Exit {
    link_id: u16,
    exit: u32
}

#[derive(Clone)]
pub struct Junction {
    id:u32,
    links: Vec<Rc<RefCell<Exit>>>
}

impl Junction {
    pub fn reciprocal(entry: u32) -> u32 {
        let mut value = entry + 180;

        while value>=360 {
            value -= 360
        }
        return value;
    }

    pub fn normalise_exit(input: i32) -> u32 {
        let mut value = input;
        while value<0 {
            value += 360;
        }
        while value >= 360 {
            value -= 360;
        }
        value as u32
    }

    pub fn new(id:u32) -> Junction {
        Junction {
            id,
            links: Vec::new()
        }
    }

    pub fn find_entry(&self, heading: f64) -> usize {
        let reciprocal_heading = find_reciprocal_heading(heading);
        let mut  closest_index = 0;
        let mut closest_delta = f64::MAX;
        for i in 0..self.links.len() {
            let exit = self.links[i].borrow().exit;
            let delta = f64::abs(exit as f64 - reciprocal_heading);
            if delta < closest_delta {
                closest_delta = delta;
                closest_index = i;
            }
        }
        closest_index
    }

    pub fn find_exit_from_heading(&self, heading: f64) -> usize {
        let mut closest_delta = f64::MAX;
        let mut exit_index:usize = usize::MAX;
        let heading_hemi = hemisphere(heading as u32);
        for i in 0..self.links.len() {
            let exit = self.links[i].borrow().exit;
            let delta = f64::abs(exit as f64 - heading);
            let exit_hemi = hemisphere(exit);

            if delta < closest_delta && exit_hemi == heading_hemi {
                closest_delta = delta;
                exit_index = i;
            }
        }
        exit_index
    }

    pub fn find_relative_exit(&self, entry_index:usize, relative_exit:usize) -> usize {

        let mut exit_index:i32 = (entry_index as i32 - relative_exit as i32) % self.links.len() as i32;
        while exit_index<0 {
            exit_index += self.links.len() as i32;
        }
        exit_index as usize
    }

    pub fn find_exit_from_turn_direction(&self, entry_index:usize, turn_dir: TurnDirection) -> usize {
        let entry = find_reciprocal_heading(self.links[entry_index].borrow().exit as f64);
        let mut heading = match turn_dir {
            TurnDirection::Straight => entry,
            TurnDirection::Left => entry + 90.0,
            TurnDirection::Right => entry - 90.0,
            TurnDirection::UTurn => entry + 180.0
        };
        while heading>=360.0 {
            heading -= 360.0;
        }
        while heading < 0.0 {
            heading += 360.0;
        }

        self.find_exit_from_heading(heading as f64)
    }
    pub fn find_exit_from_compass(&self, dir: CompassDirection) -> usize {
        let heading:u32 = match dir {
            CompassDirection::North => 0,
            CompassDirection::NorthEast => 315,
            CompassDirection::East => 270,
            CompassDirection::SouthEast => 270-45,
            CompassDirection::South => 180,
            CompassDirection::SouthWest => 180 - 45,
            CompassDirection::West => 90,
            CompassDirection::NorthWest => 45
        };
        self.find_exit_from_heading(heading as f64)
    }

    // fn build_routes(&self, network:& Network, routing:&mut Routing) -> () {
    //     // Build immediately accessible hops
    //     // for exit in &self.outgoing {
    //     //     routing.hops.insert(Hop::from(self.id,
    //     //                                 LogicalAddress::new(Identifier::new(exit.link_id, 0, 0, 0), Mask::new(true,false,false,false)),
    //     //                                 LogicalAddress::new(Identifier::new(exit.link_id, 0, 0,0), Mask::new(true,false,false,false)), exit.exit));
    //     // }
    //     // for exit in &self.incoming {
    //     //     routing.hops.insert(Hop::from(self.id,
    //     //                                 LogicalAddress::new(Identifier::new(exit.link_id, 0, 0, 0), Mask::new(true,false,false,false)),
    //     //                                 LogicalAddress::new(Identifier::new(exit.link_id, 0, 0,0), Mask::new(true,false,false,false)), exit.exit));
    //     // }
    //     //
    //     // let mut reciprocals: HashSet<Hop> = HashSet::new();
    //     // for hop in &routing.hops {
    //     //     // Look at the incoming links and add a hop for the destination
    //     //     // if let Some(origin) = network.get_link(hop.destination.id.link).origin {
    //     //     //     // Add a reciprocal route
    //     //     //     for incoming in &network.get_junc(origin).incoming {
    //     //     //         reciprocals.insert(Hop::from(origin,
    //     //     //                                    LogicalAddress::new(Identifier::new(hop.destination.id.link, 0, 0, 0), Mask::new(true,false,false,false)),
    //     //     //                                    LogicalAddress::new(Identifier::new(*incoming, 0, 0, 0), Mask::new(true, false, false, false)), 90));
    //     //     //
    //     //     //     }
    //     //     //     // for outgoing in &network.get_junc(origin).outgoing {
    //     //     //     //     reciprocals.insert(Hop::from(origin,
    //     //     //     //                                 LogicalAddress::new(Identifier::new())))
    //     //     //     // }
    //     //     // }
    //     //     for outgoing in &network.get_junc(hop.junction).outgoing {
    //     //         let link = network.get_link(hop.destination.id.link);
    //     //         if let Some(origin) = link.origin {
    //     //             let mut found = false;
    //     //             for hop2 in &routing.hops {
    //     //                 if hop2.junction == origin && hop2.destination.id.link == outgoing.link_id {
    //     //                     found = true;
    //     //                 }
    //     //             }
    //     //             if !found {
    //     //                 reciprocals.insert(Hop::from(origin,
    //     //                                              LogicalAddress::new(Identifier::new(outgoing.link_id, 0, 0, 0), Mask::new(true, false, false, false)),
    //     //                                              hop.destination, outgoing.exit));
    //     //             }
    //     //         }
    //     //     }
    //     // }
    //     // routing.hops = &routing.hops|&reciprocals;
    // }

    fn from_query(id:u32) -> Junction {
        Junction {
            id,
            links:Vec::new()
        }
    }

    pub fn num_links(&self) -> usize {
        self.links.len()
    }


    pub fn add_link(&mut self, id:u16, exit_id:u32) {
        self.links.push(Rc::new(RefCell::new(Exit{link_id:id,exit:exit_id})));
    }
}
pub struct Link {
    id:u16,
    tiles: Vec<u16>,
    origin: Option<u32>,
    destination: Option<u32>
}

impl<'a> Link {
    pub fn new(id:u16) -> Link {
        Link {
            id,
            tiles:Vec::new(),
            origin:None,
            destination:None
        }
    }

    fn from_query(id: u16, origin:u32, destination:u32) -> Link {
        Link {
            id,
            tiles:Vec::new(),
            origin:Some(origin),
            destination:Some(destination)
        }
    }
}

#[derive(PartialEq, Debug, Copy, Clone)]
pub enum TurnDirection {
    Left,
    Right,
    Straight,
    UTurn
}


#[derive(PartialEq, Debug, Copy, Clone)]
pub enum CompassDirection {
    North,
    NorthEast,
    East,
    SouthEast,
    South,
    SouthWest,
    West,
    NorthWest
}

#[derive(PartialEq, Debug)]
pub enum Turn {
    Relative(TurnDirection),
    Compass(CompassDirection),
    Exit(u8),
    Heading(u32)
}

use std::str::FromStr;

impl FromStr for TurnMultiplicity {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let parts: Vec<&str> = s.split(':').collect();

        match parts.as_slice() {
            ["Count", count] => {
                let count:u32 = count.parse().unwrap();
                Ok(TurnMultiplicity::Count(count))
            }
            ["Always"] => {
                Ok(TurnMultiplicity::Always)
            }
            _ => Err(format!("invalid turn multiplicity {}", s)),
        }
    }
}
impl FromStr for TurnDirection {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "Left" => Ok(TurnDirection::Left),
            "Right" => Ok(TurnDirection::Right),
            "Straight" => Ok(TurnDirection::Straight),
            "UTurn" => Ok(TurnDirection::UTurn),
            _ => Err(format!("invalid turn direction: {}", s))
        }
    }
}

impl FromStr for CompassDirection {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "North" => Ok(CompassDirection::North),
            "NorthEast" => Ok(CompassDirection::NorthEast),
            "East" => Ok(CompassDirection::East),
            "SouthEast" => Ok(CompassDirection::SouthEast),
            "South" => Ok(CompassDirection::South),
            "SouthWest" => Ok(CompassDirection::SouthWest),
            "West" => Ok(CompassDirection::West),
            "NorthWest" => Ok(CompassDirection::NorthWest),
            _ => Err(format!("invalid compass direction: {}", s))
        }
    }
}
impl FromStr for Turn {
    type Err = String;  // or use a custom error type

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let parts: Vec<&str> = s.split(':').collect();

        match parts.as_slice() {
            [which, direction] => {

                match which {
                    &"Relative" => {
                        let dir = direction.parse().unwrap();
                        Ok(Turn::Relative(dir))
                    }
                    &"Compass" => {
                        let dir:CompassDirection = direction.parse().unwrap();
                        Ok(Turn::Compass(dir))
                    }
                    &"Exit" => {
                        let dir:u8 = direction.parse().unwrap();
                        Ok(Turn::Exit(dir))
                    }
                    &"Heading" => {
                        let dir:u32 = direction.parse().unwrap();
                        Ok(Turn::Heading(dir))
                    }
                    _ => {
                        Err("Invalid turn".to_string())
                    }
                }
            }
            _ => Err("Invalid Turn format".to_string()),
        }
    }
}
#[derive(PartialEq, Debug)]
pub enum TurnMultiplicity {
    Count(u32),
    Always
}

#[derive(PartialEq, Debug)]
pub struct TurningPattern {
    turn:Turn,
    count:TurnMultiplicity
}

impl FromStr for TurningPattern {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let parts: Vec<&str> = s.split_whitespace().collect();

        match parts.as_slice() {
            [turn, multiplicity] => {
                Ok(TurningPattern { turn:turn.parse().unwrap(), count: multiplicity.parse().unwrap() })
            }
            _ => Err(format!("invalid turn pattern: {}", s))
        }
    }
}
#[derive(PartialEq, Debug)]
pub struct Route {
    start_link:u16,
    offset:f64,
    distance:f64,
    trav_dir:i32,
    patterns:Vec<TurningPattern>
}

#[derive(Copy, Clone)]
pub enum RouteParsing {
    ParsingStartLink,
    ParsingSpace,
    ParsingOffset,
    ParsingDistance,
    ParsingTravDir,
    ParsingTurnPattern,
    ParsingFinished
}
impl Route {
    pub fn empty() -> Route {
        Route {
            start_link:0,
            offset:0.0,
            distance:0.0,
            trav_dir:1,
            patterns:vec![]
        }
    }
    pub fn parse(input:&str) -> Route {
        let mut start = 0;
        let mut end = 0;
        let input = input.trim_start();
        let mut state = RouteParsing::ParsingStartLink;
        let mut retval : Route = Route::empty();
        let mut next_state : RouteParsing = RouteParsing::ParsingStartLink;
        for c in input.chars() {
            match state {
                RouteParsing::ParsingStartLink => {
                    if !c.is_whitespace() {
                        end += 1;
                    }
                    else {
                        retval.start_link = input[0..end].parse::<u16>().unwrap_or(0);
                        start = end+1;
                        end = start;
                        state = RouteParsing::ParsingSpace;
                        next_state = RouteParsing::ParsingOffset;
                    }
                }
                RouteParsing::ParsingSpace => {
                    if c.is_whitespace() {
                        start += 1;
                    }
                    else {
                        state = next_state;
                        end = start;
                    }
                }
                RouteParsing::ParsingOffset => {
                    if !c.is_whitespace() {
                        end+=1;
                    }
                    else {
                        retval.offset = input[start..=end].trim_start().parse::<f64>().unwrap_or(0.0);
                        start = end+2;
                        end = start;
                        state = RouteParsing::ParsingSpace;
                        next_state = RouteParsing::ParsingDistance;
                    }
                }
                RouteParsing::ParsingDistance => {
                    if !c.is_whitespace() {
                        end+=1;
                    }
                    else {
                        retval.distance = input[start..=end].trim_start().parse::<f64>().unwrap_or(0.0);
                        start = end+2;
                        state = RouteParsing::ParsingSpace;
                        next_state = RouteParsing::ParsingTravDir;
                    }
                }
                RouteParsing::ParsingTravDir => {
                    if !c.is_whitespace() {
                        end+=1;
                    }
                    else {
                        retval.trav_dir = input[start..=end].trim_start().parse::<i32>().unwrap_or(0);
                        start = end+2;
                        state = RouteParsing::ParsingSpace;
                        next_state = RouteParsing::ParsingTurnPattern;
                    }
                }
                RouteParsing::ParsingTurnPattern => {
                    let parts = input[start..].split_whitespace().collect::<Vec<_>>();
                    for chunk in parts.chunks(2) {
                        println!("{:?}",chunk);
                        let input = chunk.join(" ");
                        println!("{}",input);
                        let turn  = input.parse::<TurningPattern>();
                        if let Ok(turn) = turn {
                            retval.patterns.push(turn);
                        }

                    }
                    state = RouteParsing::ParsingFinished;

                }
                RouteParsing::ParsingFinished => {
                    // Do nothing.
                }
            }
        }
        match state {
            RouteParsing::ParsingDistance => {
                retval.distance = input[start..=end].trim_start().parse::<f64>().unwrap_or(0.0);
            }
            RouteParsing::ParsingTurnPattern => {
                let turn = input[start..=end].trim_start().parse::<TurningPattern>();
                if let Ok(turn) = turn {
                    retval.patterns.push(turn);
                }
            }
            _ => {

            }
        }
        retval
    }
}
#[derive(Copy, Clone)]
#[derive(Eq, Hash, PartialEq)]
pub struct Hop {
    junction: u32,
    dest_junc:u32,
    // destination: LogicalAddress,
    // next_hop: LogicalAddress,
    exit: u32
}

pub struct Routing {
    hops: HashSet<Hop>,
}

impl Hop {
    pub fn from(junction:u32, dest_junc:u32, exit:u32) -> Hop {
        Hop {
            junction,
            dest_junc,
            exit
        }
    }
}
impl Routing {
    pub fn new() -> Routing {
        Routing {
            hops: HashSet::new(),
        }
    }
}

#[derive(Clone)]
pub struct SpanningNode {
    children: Vec<Rc<RefCell<SpanningNode>>>,
    parent: Weak<RefCell<SpanningNode>>,
    value:Weak<RefCell<Junction>>
}

impl SpanningNode {
    pub fn new(parent:Weak<RefCell<SpanningNode>>, junc:Weak<RefCell<Junction>>) -> SpanningNode {

        SpanningNode {
            children:vec![],
            parent: parent,
            value: junc
        }
    }

    pub fn empty() -> SpanningNode {
        SpanningNode {
            children:vec![],
            parent: Weak::new(),
            value: Weak::new()
        }
    }

    pub fn num_nodes(&self) -> usize {
        let retval:usize = 0;
        self.num_nodes_helper(retval)
    }

    fn num_nodes_helper(&self, count:usize) -> usize {
        let mut retval:usize = count+1;
        for child in &self.children {
            retval += child.borrow().num_nodes();
        }
        retval
    }

    pub fn depth_first_traversal<NodeFunc>(node:Rc<RefCell<SpanningNode>>, node_func:&NodeFunc) -> ()
    where NodeFunc : Fn(Rc<RefCell<SpanningNode>>)
    {
        node_func(node.clone());
        for child in &node.borrow().children {
            Self::depth_first_traversal(child.clone(), node_func);
        }
    }
}

pub struct Network {
    links : Vec<Box<Link>>,
    junctions : Vec<Rc<RefCell<Junction>>>,
    tiles: Vec<Box<Tile>>,
    segments: Vec<Box<Segment>>,
    // One for each Junction
    routing: RefCell<Routing>,
    spanning_tree: Rc<RefCell<SpanningNode>>
}

impl<'a> Network {
    pub fn new(links:Vec<Box<Link>>, junctions:Vec<Rc<RefCell<Junction>>>) -> Network {
        Network {
            links,
            junctions,
            tiles: Vec::new(),
            segments: Vec::new(),
            routing:RefCell::new(Routing::new()),
            spanning_tree: Rc::new(RefCell::new(SpanningNode::empty()))
        }
    }

    pub fn from(connection:&Connection) -> Network {
        let link_gw:LinkGateway = LinkGateway::new(connection);
        let junc_gw:JunctionGateway = JunctionGateway::new(connection);
        let tile_gw: TileGateway = TileGateway::new(connection);
        let seg_gw : SegmentGateway = SegmentGateway::new(connection);
        let mut network = Network::empty();
        network.set_links(link_gw.find_all().unwrap_or(Vec::new()));
        network.set_junctions(junc_gw.find_all().unwrap_or(Vec::new()));
        network.set_junction_connections(&mut junc_gw.find_connections().unwrap_or(Vec::<(u32,u16,u32)>::new()));
        network.set_tiles(tile_gw.find_all().unwrap_or(Vec::new()));
        network.set_segments(seg_gw.find_all().unwrap_or(Vec::new()));
        network.build_spanning_tree();
        network.build_routes();
        network
    }

    pub fn first_segment_for_link(&self, link:&Link) -> Option<&Segment> {
        for tile in &self.tiles {
            if tile.link == link.id {
                for segment in &self.segments {
                    if segment.tile == tile.id {
                        return Some(segment);
                    }
                }
            }
        }
        return None;
    }

    pub fn last_segment_for_link(&self, link:&Link) -> Option<&Segment> {
        let mut retval:Option<&Segment> = None;
        for tile in &self.tiles {
            if tile.link == link.id {
                for segment in &self.segments {
                    if segment.tile == tile.id {
                        retval = Some(segment);
                    }
                }
            }
        }
        retval
    }

    pub fn find_exit_by_heading(&self, to: &Junction, exit_heading: u32) -> usize {
        let mut exit_index = 0;
        for _ in 0..self.links.len() {
            let exit = &to.links[exit_index];
            if exit.borrow().exit == exit_heading {
                return exit_index;
            }
            exit_index = (exit_index+1) % self.links.len();
        }

        return exit_index;
    }

    pub fn find_exit(&self, from:&Junction, to:&Junction) -> usize {
        // let from = from.upgrade().unwrap().clone().borrow();
        // let to = to.upgrade().unwrap().clone().borrow();
        for i in 0..from.links.len() {
            let exit = from.links[i].borrow();
            let link = self.get_link(exit.link_id);
            if let Some(origin) = link.origin {
                if let Some(dest) = link.destination {
                    if self.get_junc(origin).borrow().id == from.id && self.get_junc(dest).borrow().id == to.id {
                        return i;
                    }
                    if self.get_junc(origin).borrow().id == to.id && self.get_junc(dest).borrow().id == from.id {
                        return i;
                    }
                }
            }
        }
        return usize::max_value();
    }

    fn dummy(&self, junc:&Junction, link:&Link, exit:u32, dest_junc:u32) -> () {
        println!("{} {} {} {}", junc.id, link.id, exit, dest_junc);
    }

    pub fn evaluate_route(&self, route:&Route) -> Vec<(u32, usize)> {
        let mut v = Vec::new();
        let mut pos = LogicalCoord::empty();
        pos.offset = route.offset;
        pos.distance = route.distance;
        let mut link = self.get_link(route.start_link);
        let mut trav_dir = route.trav_dir;
        for i in 0..route.patterns.len() {
            let mut num_turns:u32 = u32::MAX;
            match route.patterns[i].count {
                TurnMultiplicity::Count(count) => {
                    num_turns = count;
                }
                _ => {
                    // Do nothing yet.
                }

            }
            let mut turn_num = 0;
            loop {
                let mut junc = link.destination;
                let mut incoming_heading = 0.0;
                if trav_dir == -1 {
                    if let Some(segment) = self.first_segment_for_link(link) {
                        incoming_heading = find_reciprocal_heading(segment.h);
                    }
                    junc = link.origin;
                }
                else {
                    if let Some(segment) = self.last_segment_for_link(link) {
                        incoming_heading = segment.h;
                    }
                }
                if let Some(upcoming_junc) = junc {
                    let upcoming_junc = self.get_junc(upcoming_junc);
                    let entry = upcoming_junc.borrow().find_entry(incoming_heading);
                    let mut exit_index = usize::MAX;
                    match &route.patterns[i].turn {
                        Turn::Relative(dir) => {
                            exit_index = upcoming_junc.borrow().find_exit_from_turn_direction(entry, *dir);
                        }
                        Turn::Compass(dir) => {
                            exit_index = upcoming_junc.borrow().find_exit_from_compass(*dir);
                        }
                        Turn::Exit(relative_exit) => {
                            exit_index = upcoming_junc.borrow().find_relative_exit(entry, *relative_exit as usize)
                        }
                        Turn::Heading(heading) => {
                            exit_index = upcoming_junc.borrow().find_exit_from_heading(*heading as f64)
                        }
                    }
                    if exit_index != usize::MAX {
                        v.push((upcoming_junc.borrow().id, exit_index));
                        let exit = upcoming_junc.borrow().links[exit_index].clone();
                        link = self.get_link(exit.borrow().link_id);
                        if let Some(origin) = link.origin {
                            if origin == upcoming_junc.borrow().id {
                                trav_dir = 1;
                            }
                        }
                        if let Some(destination) = link.destination {
                            if destination == upcoming_junc.borrow().id {
                                trav_dir = -1;
                            }
                        }
                    }
                    else {
                        break;
                    }
                    turn_num += 1;
                    if turn_num == num_turns {
                        break;
                    }
                }
            }
        }
        v
    }

    fn build_routes(&mut self) {
        // for junc in &self.junctions {
        //     junc.build_routes(self, &mut self.routing.borrow_mut());
        // }
        // let print_step = |junc:Rc<RefCell<Junction>>, link:&Link, exit:u32, dest_junc:u32, path:&Vec<(u32,u32)>| {
        //     // self.routing.borrow_mut().hops.insert(Hop::from(junc.id,
        //     //                                                 LogicalAddress::new(Identifier::new(link.id, 0, 0, 0), Mask::new(true, false, false, false)),
        //     //                                                 LogicalAddress::new(Identifier::new(link.id, 0, 0, 0), Mask::new(true, false, false, false)),
        //     //                                                 exit
        //     // )
        //     // );
        //     // For each outgoing link reachable directly from dest_junc, add a route from origin to origin via link
        //     //let dest_junc = self.get_junc(dest_junc);
        //     // for outgoing_exit in &dest_junc.outgoing {
        //     //     let outgoing_link = self.get_link(outgoing_exit.link_id);
        //     //     self.routing.borrow_mut().hops.insert(Hop::from(junc.id,
        //     //     LogicalAddress::new(Identifier::new(outgoing_link.id, 0, 0, 0), Mask::new(true, false, false, false)),
        //     //     LogicalAddress::new(Identifier::new(link.id, 0, 0, 0), Mask::new(true, false, false, false)),
        //     //     exit
        //     //     ));
        //     //     println!("Add route: {} {} {} {}", junc.id, outgoing_exit.link_id, link.id, exit);
        //     // }
        //     if let Some(last_junc) = path.last() {
        //         let last_junc = self.get_junc(last_junc.0);
        //
        //         if last_junc.borrow().links.is_empty() {
        //
        //             // Iterate over path, adding routes
        //             for i in 0..path.len() {
        //                 println!("path: junc {} exit {}", path[i].0, path[i].1);
        //                 let src_junc = self.get_junc(path[i].0);
        //                 for j in i+1..path.len() {
        //                     let dest_junc = self.get_junc(path[j].0);
        //                     if path[i].0 != path[j].0 && path[i].1 != 270 {
        //                         //println!("origin_junc: {} dest_junc: {} exit {}", src_junc.id, dest_junc.id, path[i].1);
        //
        //                         println!("Add route from {} to {} via {} exit {}", src_junc.borrow().id, dest_junc.borrow().id, path[i].0, path[i].1);
        //                         self.routing.borrow_mut().hops.insert(Hop::from(src_junc.borrow().id, dest_junc.borrow().id, path[i].1));
        //                     }
        //                 }
        //             }
        //         }
        //     }
        // };
        // self.depth_first_traversal(&print_step, |junc:Rc<RefCell<Junction>>| println!("{}", junc.borrow().id));
        let build = |node:Rc<RefCell<SpanningNode>>| {
            if node.borrow().children.is_empty() {
                let mut root:Weak<RefCell<SpanningNode>> = Rc::downgrade(&node);
                let mut path:Vec<Rc<RefCell<SpanningNode>>> = vec![];
                while let Some(parent) = root.upgrade() {
                    root = parent.borrow().parent.clone();
                    path.push(parent);
                }
                path.reverse();
                for i in 0..path.len() {
                    let src_junc = &path[i].borrow().value.upgrade().clone().unwrap().borrow().clone();
                    println!("path: junc {}", src_junc.id);
                    if i+1<path.len() {
                        let next_hop = &path[i + 1].borrow().value.upgrade().clone().unwrap().borrow().clone();
                        let exit_index = self.find_exit(src_junc, next_hop);
                        if exit_index != usize::max_value() {
                            let exit = src_junc.links[exit_index].clone();
                            self.routing.borrow_mut().hops.insert(Hop::from(src_junc.id, next_hop.id, exit.borrow().exit));
                            for j in i + 2..path.len() {
                                let dest_junc = &path[j].borrow().value.upgrade().unwrap().borrow().clone();
                                if src_junc.id != dest_junc.id && exit.borrow().exit != 270 {
                                    //println!("origin_junc: {} dest_junc: {} exit {}", src_junc.id, dest_junc.id, path[i].1);

                                    println!("Add route from {} to {} via {} exit {}", src_junc.id, dest_junc.id, src_junc.id, exit.borrow().exit);
                                    self.routing.borrow_mut().hops.insert(Hop::from(src_junc.id, dest_junc.id, exit.borrow().exit));
                                }
                            }
                        } else {
                            println!("Warning team:No exit from {} to {}", src_junc.id, next_hop.id);
                        }
                    }
                }
            }
        };
        SpanningNode::depth_first_traversal(self.spanning_tree.clone(),&build);
    }

    fn build_spanning_tree(&mut self) -> () {
        let parent_stack:RefCell<Vec<Rc<RefCell<SpanningNode>>>> = RefCell::from(Vec::new());
        parent_stack.borrow_mut().push(Rc::from(RefCell::from(SpanningNode::new(Weak::new(), Rc::downgrade(&(self.junctions[0].clone()))))));
        let build = |junc:Rc<RefCell<Junction>>| {//, link:&Link, exit:u32, dest_junc:u32, path:&Vec<(u32,u32)>| {
            let mut parent_stack = parent_stack.borrow_mut();
            if let Some(top) = parent_stack.deref().last() {
                let child = Rc::from(RefCell::new(SpanningNode::new(Rc::downgrade(&top.clone()), Rc::downgrade(&junc.clone()))));
                top.borrow_mut().children.push(child.clone());
                parent_stack.push(child.clone());
            }
        };
        if let Some(root) = parent_stack.borrow_mut().last() {
            self.spanning_tree = root.clone();
        }
        let empty = |junc:Rc<RefCell<Junction>>, link:&Link, exit:u32, origin:u32, path:&Vec<(u32,u32)>| {
        };
        self.depth_first_traversal(&empty, &build);
    }

    fn depth_first_traversal_helper<LinkFunc, JuncFunc>(& self, junc:Rc<RefCell<Junction>>, visited:&mut HashSet<u32>, path: &mut Vec<(u32,u32)>, link_func:&LinkFunc, junc_func:&JuncFunc) -> ()
    where LinkFunc : Fn(Rc<RefCell<Junction>>, &Link, u32, u32, &Vec<(u32,u32)>),
        JuncFunc: Fn(Rc<RefCell<Junction>>)
    {
        if !visited.contains(&junc.borrow().id) {
            visited.insert(junc.borrow().id);
            for exit in &junc.borrow().links {
                let link = self.get_link(exit.borrow().link_id);
                let dest_junc = link.destination;
                if let Some(origin) = link.origin && dest_junc.is_some() {
                    path.push((dest_junc.unwrap(),exit.borrow().exit));
                    let destination = self.get_junc(dest_junc.unwrap());
                    let origin = self.get_junc(origin);
                    if !visited.contains(&destination.borrow().id) {
                        junc_func(destination.clone());
                        link_func(destination.clone(), link, exit.borrow().exit, origin.borrow().id, path);
                        self.depth_first_traversal_helper(destination, visited, path, link_func, junc_func);
                    }
                }
            }

            path.pop();
        }
    }

    pub fn depth_first_traversal<LinkFunc, JuncFunc>(&self, link_func:&LinkFunc, junc_func:JuncFunc) -> ()
    where LinkFunc: Fn(Rc<RefCell<Junction>>, &Link, u32, u32, &Vec<(u32,u32)>),
        JuncFunc: Fn(Rc<RefCell<Junction>>)
    {
        let mut visited: HashSet<u32> = HashSet::new();
        let mut path:Vec<(u32,u32)> = Vec::new();
        if !self.junctions.is_empty() {
            let junc = self.get_junc(1);
            self.depth_first_traversal_helper(junc, &mut visited, &mut path, link_func, &junc_func);
        }
    }

    pub fn empty() -> Network {
        Network {
            links:Vec::new(),
            junctions:Vec::new(),
            tiles: Vec::new(),
            segments:Vec::new(),
            routing:RefCell::new(Routing::new()),
            spanning_tree:Rc::new(RefCell::from(SpanningNode::empty()))
        }
    }

    pub fn route(&self, junc_id: u32, src_junc:u32, dest_junc:u32, to_dest:bool) -> Option<Hop> {
        let src_junc = self.get_junc(src_junc);
        // let origin = src_link.origin;
        // let dest = src_link.destination;

        for hop in &self.routing.borrow().hops {
            let junc = self.get_junc(hop.junction);
            let dest = self.get_junc(hop.dest_junc);
            if  junc.borrow().id == junc_id && dest.borrow().id == dest_junc && to_dest {
                return Some(*hop);
            }
            if junc.borrow().id == junc_id && dest.borrow().id == src_junc.borrow().id && !to_dest {
                return Some(*hop);
            }
        }
        None
    }

    pub fn get_link(&self, id:u16) -> &Link {
        &self.links[(id-1) as usize]
    }

    pub fn get_link_mut(&mut self, id:u16) -> &mut Link {
        &mut self.links[(id-1) as usize]
    }

    pub fn add_link(&mut self, link:Box<Link>) {
        self.links.push(link);
    }

    pub fn set_links(&mut self, links:Vec<Box<Link>>) {
        self.links = links;
    }

    pub fn set_junctions(&mut self, junctions:Vec<Rc<RefCell<Junction>>>) {
        self.junctions = junctions;
    }

    pub fn set_tiles(&mut self, tiles:Vec<Box<Tile>>) {
        self.tiles = tiles;
    }
    pub fn set_junction_connections(&mut self, connections: &mut Vec<(u32, u16, u32)>) {
        for connection in connections {
        self.get_junc_mut(connection.0).borrow_mut().add_link(connection.1, connection.2);
        }
    }

    pub fn set_segments(&mut self , segments:Vec<Box<Segment>>) {
        self.segments = segments;
    }

    pub fn num_links(&self) -> usize {
        self.links.len()
    }

    pub fn num_junctions(&self) -> usize {
        self.junctions.len()
    }

    pub fn get_junc_mut(&mut self, id:u32) -> Rc<RefCell<Junction>> {
        self.junctions[(id - 1) as usize].clone()
    }

    pub fn get_junc(&self, id:u32) -> Rc<RefCell<Junction>> {
        self.junctions[(id-1) as usize].clone()
    }

    pub fn get_junc_if_exists(&self, id: Option<u32>) -> Option<Rc<RefCell<Junction>>> {
        if let Some(valid_id) = id {
            Some(self.get_junc(valid_id))
        }
        else {
            None
        }
    }
    pub fn get_junc_if_exists_mut(&mut self, id: Option<u32>) -> Option<Rc<RefCell<Junction>>> {
        if let Some(valid_id) = id {
            Some(self.get_junc_mut(valid_id))
        }
        else {
            None
        }
    }

    pub fn num_tiles(&self) -> usize {
        self.tiles.len()
    }

    pub fn num_segments(&self) -> usize {
        self.segments.len()
    }
}

pub struct NetworkBuilder {
    links:Vec<Box<Link>>,
    junctions:Vec<Rc<RefCell<Junction>>>,
    next_junc:u32,
    next_link:u16
}

impl<'a> NetworkBuilder {
    pub fn new() -> NetworkBuilder {
        NetworkBuilder {
            links:Vec::new(),
            junctions:Vec::new(),
            next_junc:0,
            next_link:0
        }
    }

    pub fn create_link(&mut self) {
        self.links.push(Box::new(Link::new(self.next_link)));
        self.next_link+=1;
        if let Some(j) = self.junctions.last_mut() {
            j.borrow_mut().links.push(Rc::new(RefCell::new(Exit{link_id:self.links.last().unwrap().id,exit:90})));
        }
    }

    pub fn add_junction(&mut self) {
        self.junctions.push(Rc::new(RefCell::from(Junction::new(self.next_junc))));
        self.next_junc += 1;
    }

    pub fn add_straight(&mut self, _:InertialCoord, _:f64) {

    }

    pub fn build(self) -> Box<Network> {
        Box::new(Network::new(self.links, self.junctions))
    }
}

struct LinkGateway<'a> {
    connection: &'a Connection,

}

impl<'a> LinkGateway<'a> {
    pub fn new(connection: &'a Connection) ->  LinkGateway<'a> {
        LinkGateway {
            connection
        }
    }

    pub fn find_all(&self) -> Result<Vec<Box<Link>>, Error> {
        let statement = self.connection.prepare("SELECT * FROM links;");
        if let  Err(e) = statement {
            return Err(e);
        }
        let mut statement = statement.unwrap();
        let link_iter = statement.query_map([], |row| {
            Ok(Link::from_query(row.get(0).unwrap(), row.get(1).unwrap(), row.get(2).unwrap()))
        });
        let mut links = Vec::new();
        for link in link_iter.unwrap() {
            links.push(Box::new(link.unwrap()));
        }
        Ok(links)
    }
}

struct JunctionGateway<'a> {
    connection: & 'a Connection,
}

impl<'a> JunctionGateway<'a> {
    pub fn new(connection: &'a Connection) -> JunctionGateway<'a> {
        JunctionGateway {
            connection
        }
    }
    pub fn find_all(&self) -> Result<Vec<Rc<RefCell<Junction>>>, Error> {
        let mut statement = self.connection.prepare("SELECT * FROM junctions;");
        if let  Err(e) = statement {
            return Err(e);
        }
        let mut statement = statement.unwrap();
        let junc_iter = statement.query_map([], |row| {
            Ok(Junction::from_query(row.get(0).unwrap()))
        });
        let mut juncs:Vec<Rc<RefCell<Junction>>> = Vec::new();
        for junc in junc_iter.unwrap() {
            juncs.push(Rc::new(RefCell::from(junc.unwrap())));
        }
        Ok(juncs)
    }

    pub fn find_connections(&self) -> Result<Vec<(u32,u16,u32)>, Error> {
        let mut statement = self.connection.prepare("SELECT * FROM junctions_links ORDER BY junc_id, exit;");
        if let  Err(e) = statement {
            return Err(e);
        }
        let mut statement = statement.unwrap();
        let connection_iter = statement.query_map([], |row| {
            Ok((row.get::<usize, u32>(0).unwrap() as u32, row.get::<usize,u16>(1).unwrap(), row.get::<usize,u32>(2).unwrap()))
        });
        let mut connections = Vec::new();
        for connection in connection_iter.unwrap() {
            let connection = connection.unwrap();
            connections.push(connection);
        }
        Ok(connections)
    }
}

struct TileGateway<'a> {
    connection: &'a Connection,
}

impl<'a> TileGateway<'a> {
    pub fn new(connection: &'a Connection) -> TileGateway<'a> {
        TileGateway {
            connection
        }
    }
    pub fn find_all(&self) -> Result<Vec<Box<Tile>>, Error> {
        let statement = self.connection.prepare("SELECT * FROM tiles;");
        if let  Err(e) = statement {
            return Err(e);
        }
        let mut statement = statement.unwrap();
        let tile_iter = statement.query_map([], |row| {
            Ok(Tile::from_query(row.get(0).unwrap(), row.get(1).unwrap()))
        });
        let mut tiles = Vec::new();
        for tile in tile_iter.unwrap() {
            tiles.push(Box::new(tile.unwrap()));
        }
        Ok(tiles)
    }
}

struct SegmentGateway<'a> {
    connection: &'a Connection
}

impl<'a> SegmentGateway<'a> {
    pub fn new(connection: &Connection) -> SegmentGateway<'_> {
        SegmentGateway {
            connection
        }
    }

    pub fn find_all(&self) -> Result<Vec<Box<Segment>>, Error> {
        let mut statement = self.connection.prepare("SELECT * FROM segments;");
        if let  Err(e) = statement {
            return Err(e);
        }
        let mut statement = statement.unwrap();
        let seg_iter = statement.query_map([], |row| {
            Ok(Segment::from_query(row))
        });
        let mut segments = Vec::new();
        for segment in seg_iter.unwrap() {
            segments.push(Box::new(segment.unwrap()));
        }
        Ok(segments)
    }
}

pub fn find_reciprocal_heading(heading:f64) -> f64 {
    let mut reciprocal_heading:f64 = heading + 180.0;
    while reciprocal_heading >= 360.0 {
        reciprocal_heading -= 360.0;
    }
    reciprocal_heading
}

pub fn hemisphere(input:u32) -> u32 {
    let mut angle = input;
    while angle >= 360 {
        angle -= 360;
    }
    if angle < 90 || (angle >= 270 && angle < 360) {
        return 0;
    }
    1
}

#[cfg(test)]
mod tests {
    use std::ops::Deref;
    use rstest::rstest;
    use rusqlite::Connection;
    use super::*;
    use crate::math::{Curve, Identifier, InertialCoord, LogicalAddress, LogicalCoord, Mask, Network, NetworkBuilder};

    #[test]
    fn test_inertial_coords() {
        let sut = InertialCoord::new(1.0, 2.0, 3.0);
        assert_eq!(sut.x, 1.0);
        assert_eq!(sut.y, 2.0);
        assert_eq!(sut.z, 3.0);
    }

    #[test]
    fn test_logical_coords() {
        let sut = LogicalCoord::new(LogicalAddress::new(Identifier::new(1,1,1,0),Mask::new(true,true,true,false)), 1.0, 2.0, 3.0);
        assert_eq!(sut.offset, 1.0);
        assert_eq!(sut.distance, 2.0);
        assert_eq!(sut.loft, 3.0);
    }

    #[rstest]
    #[case(-1.825, 50.0, 0.0)]
    fn test_logical_to_inertial_coords(#[case] _offset: f64, #[case] _distance: f64, #[case] _loft: f64) {
        let sut = Curve::new();
        let logical = LogicalCoord::new(LogicalAddress::new(Identifier::new(1,1,1,0),Mask::new(true,true,true,false)), -1.825, 50.0, 0.0);
        let mut inertial = InertialCoord::new(0.0, 0.0, 0.0);
        sut.logical_to_inertial(&logical, &mut inertial);
        assert_eq!(inertial.x, -1.825);
        assert_eq!(inertial.y, 50.0);
        assert_eq!(inertial.z, 0.0);
    }

    #[rstest]
    #[case(-1.825, 50.0, 0.0)]
    fn test_inertial_to_logical(#[case] x: f64, #[case] y: f64, #[case] z: f64) {
        let sut = Curve::new();
        let mut logical = LogicalCoord::empty();
        let inertial = InertialCoord::new(x, y, z);
        sut.inertial_to_logical(&inertial, &mut logical);
        assert_eq!(logical.offset, -1.825);
        assert_eq!(logical.distance, 50.0);
        assert_eq!(logical.loft, 0.0);
    }

    #[rstest]
    #[case("1.1.1.0/1.1.1.0", Ok(LogicalAddress::new(Identifier::new(1,1,1,0),Mask::new(true,true,true,false))))]
    #[case("2.10.2.1/1.1.1.1", Ok(LogicalAddress::new(Identifier::new(2,10,2,1),Mask::new(true,true,true,true))))]
    #[case("2.10.2.-1/1.1.1.1", Ok(LogicalAddress::new(Identifier::new(2,10,2,-1),Mask::new(true,true,true,true))))]
    #[case("-2.10.2.-1/1.1.1.1", Err("Expected whole number, got minus sign"))]
    #[case("2.10.2.-1/2.1.1.1", Ok(LogicalAddress::new(Identifier::new(2,10,2,-1),Mask::new(true,true,true,true))))]
    #[case("2.10.2.-1", Ok(LogicalAddress::new(Identifier::new(2,10,2,-1),Mask::new(true,true,true,true))))]
    #[case("", Err("Expected some content before the '/'"))]
    #[case("/", Err("Expected some content before the '/'"))]
    #[case("/1.1.1.1", Err("Expected some content before the '/'"))]
    fn test_parse_logical_address(#[case] str: &str, #[case] addr: Result<LogicalAddress, &str>) {
        assert_eq!(LogicalAddress::parse(str),addr);
    }

    #[test]
    fn test_network_builder_add() {
        let mut sut = NetworkBuilder::new();
        sut.add_junction();
        assert_eq!(sut.junctions.len(), 1);
        sut.create_link();
        sut.add_straight(InertialCoord::new(0.0, 0.0, 0.0), 252.0);
        let network = sut.build();
        assert_eq!(1,network.num_links());
    }

    #[rstest]
    #[case("data/tests/LoadFromDB/onelink.db", 1)]
    #[case("data/tests/LoadFromDB/onelink.db", 1)]
    #[case("data/tests/LoadFromDB/twolinks.db", 2)]
    #[case("data/tests/LoadFromDB/twolinks.db", 2)]
    fn test_create_network_from_db_links(#[case] dbfile:&str, #[case] num_links:usize) {
        let connection = Connection::open(dbfile).unwrap_or_else(|e| panic!("failed to open {}: {}", dbfile, e));
        let network = Network::from(&connection);
        assert_eq!(num_links, network.num_links());
    }

    #[rstest]
    #[case("data/tests/LoadFromDB/onelink.db", 2, 1, 1)]
    #[case("data/tests/LoadFromDB/onelink.db", 2, 2, 1)]
    #[case("data/tests/LoadFromDB/twolinks.db", 3, 2, 2)]
    #[case("data/tests/LoadFromDB/twolinks.db", 3, 3, 1)]
    fn test_create_network_from_db_junctions(#[case]dbfile:&str, #[case] num_juncs:usize, #[case] junc_id:u32, #[case] num_links:usize) {
        let connection = Connection::open(dbfile).unwrap_or_else(|e| panic!("failed to open {}: {}", dbfile, e));
        let mut network = Network::from(&connection);
        assert_eq!(num_juncs, network.num_junctions());
        assert_eq!(num_links, network.get_junc_mut(junc_id).borrow().num_links());
    }

    #[rstest]
    #[case("data/tests/LoadFromDB/onelink.db", 2)]
    fn test_create_network_from_db_tiles(#[case] dbfile:&str, #[case] num_tiles:usize) {
        let connection = Connection::open(dbfile).unwrap_or_else(|e| panic!("failed to open {}: {}", dbfile, e));
        let network = Network::from(&connection);
        assert_eq!(num_tiles, network.num_tiles());
    }

    #[rstest]
    #[case("data/tests/LoadFromDB/onelink.db", 2)]
    fn test_create_network_from_db_segments(#[case] dbfile:&str, #[case] num_segments:usize) {
        let connection = Connection::open(dbfile).unwrap_or_else(|e| panic!("failed to open {}: {}", dbfile, e));
        let network = Network::from(&connection);
        assert_eq!(num_segments, network.num_segments());
    }

    #[rstest]
    #[case("data/tests/LoadFromDB/onelink.db", 1, 1, 2, true, true, 0)]
    #[case("data/tests/LoadFromDB/twolinks.db", 1, 1, 2, true, true, 0)]
    #[case("data/tests/LoadFromDB/twolinks.db", 1, 1, 3, true, true, 0)]
    fn test_routing(#[case] dbfile:&str, #[case] junc_id:u32, #[case] source_junc:u32, #[case] dest_junc: u32, #[case] to_dest:bool, #[case] exists:bool, #[case] next_exit:u32) {
        let connection = Connection::open(dbfile).unwrap_or_else(|e| panic!("failed to open {}: {}", dbfile, e));
        let network = Network::from(&connection);

        let actual = network.route(junc_id, source_junc, dest_junc, to_dest);
        assert_eq!(exists, actual.is_some());
        if let Some(actual) = actual {
            assert_eq!(dest_junc, actual.dest_junc);
            assert_eq!(next_exit, actual.exit);
        }

    }

    #[rstest]
    #[case(90, 270)]
    #[case(270, 90)]
    #[case(0, 180)]
    #[case(180, 0)]
    #[case(360, 180)]
    #[case(360+45, 45+180)]
    fn test_reciprocal_exit(#[case] entry:u32, #[case] reciprocal: u32) {
        assert_eq!(reciprocal, Junction::reciprocal(entry))
    }

    #[rstest]
    #[case(0, 0)]
    #[case(-1, 359)]
    #[case(720, 0)]
    #[case(-720, 0)]
    #[case(90, 90)]
    #[case(0, 0)]
    #[case(-45, 360-45)]
    fn test_normalise_exit(#[case] input:i32, #[case] normalised:u32) {
        assert_eq!(normalised, Junction::normalise_exit(input));
    }

    #[rstest]
    #[case("1 -1.825 200.0 1", Route {start_link:1, offset:-1.825, distance:200.0, trav_dir:1, patterns:vec![]})] //TurningPattern {turn:Turn::Relative(TurnDirection::STRAIGHT), count:TurnMultiplicity::Once}] })]
    #[case(" 1  -1.825  200.0 1", Route {start_link:1, offset:-1.825, distance:200.0, trav_dir:1, patterns:vec![]})] //TurningPattern {turn:Turn::Relative(TurnDirection::STRAIGHT), count:TurnMultiplicity::Once}] })]
    #[case("1 -1.825 200.0 1 Relative:Straight Count:1", Route {start_link:1, offset:-1.825, distance:200.0, trav_dir:1, patterns:vec![TurningPattern { turn:Turn::Relative(TurnDirection::Straight), count:TurnMultiplicity::Count(1) } ]})] //TurningPattern {turn:Turn::Relative(TurnDirection::STRAIGHT), count:TurnMultiplicity::Once}] })]
    #[case("1 -1.825 200.0 1 Relative:Straight Count:1 Compass:North Count:1", Route {start_link:1, offset:-1.825, distance:200.0, trav_dir:1, patterns:vec![TurningPattern { turn:Turn::Relative(TurnDirection::Straight), count:TurnMultiplicity::Count(1) }, TurningPattern { turn:Turn::Compass(CompassDirection::North), count:TurnMultiplicity::Count(1) } ]})] //TurningPattern {turn:Turn::Relative(TurnDirection::STRAIGHT), count:TurnMultiplicity::Once}] })]
    #[case("1 -1.825 200.0 1 Relative:Straight Count:1 Exit:2 Count:1", Route {start_link:1, offset:-1.825, distance:200.0, trav_dir:1, patterns:vec![TurningPattern { turn:Turn::Relative(TurnDirection::Straight), count:TurnMultiplicity::Count(1) }, TurningPattern { turn:Turn::Exit(2), count:TurnMultiplicity::Count(1) } ]})] //TurningPattern {turn:Turn::Relative(TurnDirection::STRAIGHT), count:TurnMultiplicity::Once}] })]
    #[case("1 -1.825 200.0 1 Relative:Straight Count:1 Heading:90 Count:1", Route {start_link:1, offset:-1.825, distance:200.0, trav_dir:1, patterns:vec![TurningPattern { turn:Turn::Relative(TurnDirection::Straight), count:TurnMultiplicity::Count(1) }, TurningPattern { turn:Turn::Heading(90), count:TurnMultiplicity::Count(1) } ]})] //TurningPattern {turn:Turn::Relative(TurnDirection::STRAIGHT), count:TurnMultiplicity::Once}] })]
    #[case("1 -1.825 200.0 1 Relative:Straight Always", Route {start_link:1, offset:-1.825, distance:200.0, trav_dir:1, patterns:vec![TurningPattern { turn:Turn::Relative(TurnDirection::Straight), count:TurnMultiplicity::Always } ]})] //TurningPattern {turn:Turn::Relative(TurnDirection::STRAIGHT), count:TurnMultiplicity::Once}] })]
    #[case("1 -1.825 200.0 1 Relative:Straight Count:1 Relative:Right Count:1", Route {start_link:1, offset:-1.825, distance:200.0, trav_dir:1, patterns:vec![TurningPattern { turn:Turn::Relative(TurnDirection::Straight), count:TurnMultiplicity::Count(1) }, TurningPattern { turn:Turn::Relative(TurnDirection::Right), count:TurnMultiplicity::Count(1) } ]})] //TurningPattern {turn:Turn::Relative(TurnDirection::STRAIGHT), count:TurnMultiplicity::Once}] })]
    fn test_parse_route(#[case] input: &str, #[case] route:Route) {
        let actual = Route::parse(input);
        assert_eq!(route, actual);
    }

    #[rstest]
    #[case("data/tests/LoadFromDB/twolinks.db", "1 -1.825 200.0 1 Relative:Straight Count:1", vec![(2, 0)])]
    #[case("data/tests/LoadFromDB/twolinks.db", "1 -1.825 200.0 1 Relative:Straight Count:1", vec![(2, 0)])]
    #[case("data/tests/LoadFromDB/fivelinks.db", "1 -1.825 200.0 1 Relative:Straight Count:2", vec![(2, 0), (3,0)])]
    #[case("data/tests/LoadFromDB/fivelinks.db", "1 -1.825 200.0 1 Relative:Left Count:1", vec![(2, 1)])]
    #[case("data/tests/LoadFromDB/fivelinks.db", "1 -1.825 200.0 1 Relative:Right Count:1", vec![(2, 3)])]
    #[case("data/tests/LoadFromDB/fivelinks.db", "1 -1.825 200.0 1 Relative:UTurn Count:1", vec![(2, 2)])]
    #[case("data/tests/LoadFromDB/fivelinks.db", "1 -1.825 200.0 1 Relative:Straight Always", vec![(2, 0), (3,0)])]
    #[case("data/tests/LoadFromDB/fivelinks.db", "1 -1.825 200.0 1 Compass:North Always", vec![(2, 0), (3,0)])]
    #[case("data/tests/LoadFromDB/fivelinks.db", "1 -1.825 200.0 1 Compass:West Always", vec![(2, 1)])]
    #[case("data/tests/LoadFromDB/fivelinks.db", "1 -1.825 200.0 1 Compass:East Always", vec![(2, 3)])]
    #[case("data/tests/LoadFromDB/fivelinks.db", "1 -1.825 200.0 1 Compass:South Always", vec![(2, 2)])]
    #[case("data/tests/LoadFromDB/fivelinks.db", "1 -1.825 200.0 1 Relative:Left Count:1", vec![(2, 1)])]
    #[case("data/tests/LoadFromDB/fivelinks.db", "1 -1.825 200.0 1 Relative:Left Always", vec![(2, 1)])]
    #[case("data/tests/LoadFromDB/fivelinks.db", "1 -1.825 200.0 1 Exit:2 Count:1", vec![(2, 0)])]
    #[case("data/tests/LoadFromDB/fivelinks.db", "1 -1.825 200.0 1 Exit:1 Count:1", vec![(2, 1)])]
    #[case("data/tests/LoadFromDB/fivelinks.db", "1 -1.825 200.0 1 Heading:0 Count:1", vec![(2, 0)])]
    #[case("data/tests/LoadFromDB/fivelinks.db", "1 -1.825 200.0 1 Heading:90 Count:1", vec![(2, 1)])]
    #[case("data/tests/LoadFromDB/fivelinks.db", "1 -1.825 200.0 1 Heading:270 Count:1", vec![(2, 3)])]
    #[case("data/tests/LoadFromDB/fivelinks.db", "1 -1.825 200.0 1 Heading:180 Count:1", vec![(2, 2)])]
    #[case("data/tests/LoadFromDB/yjunction.db", "1 -1.825 200.0 1 Heading:315 Count:1", vec![(2, 2)])]
    #[case("data/tests/LoadFromDB/twolinks.db", "2 1.825 200.0 -1 Heading:180 Count:1", vec![(2, 1)])]
    #[case("data/tests/LoadFromDB/yjunction.db", "3 1.825 200.0 -1 Heading:180 Count:1", vec![(2, 1)])]
    #[case("data/tests/LoadFromDB/fivelinks.db", "3 1.825 200.0 -1 Heading:180 Count:2", vec![(3, 1), (2, 2)])]
    #[case("data/tests/LoadFromDB/fivelinks.db", "4 1.825 200.0 -1 Compass:North Always", vec![(2, 0), (3, 0)])]
    #[case("data/tests/LoadFromDB/fivelinks.db", "4 1.825 200.0 -1 Heading:0 Always", vec![(2, 0), (3, 0)])]
    fn test_evaluate_route(#[case] dbfile: &str, #[case] input: &str, #[case] expected:Vec<(u32, usize)>) {
        let connection = Connection::open(dbfile).unwrap_or_else(|e| panic!("failed to open {}: {}", dbfile, e));
        let network = Network::from(&connection);
        let route = Route::parse(input);
        let actual = network.evaluate_route(&route);
        assert_eq!(expected, actual);
    }

    #[rstest]
    #[case("Relative:Straight", Turn::Relative(TurnDirection::Straight))]
    #[case("Compass:North", Turn::Compass(CompassDirection::North))]
    #[case("Compass:NorthEast", Turn::Compass(CompassDirection::NorthEast))]
    #[case("Compass:East", Turn::Compass(CompassDirection::East))]
    #[case("Compass:SouthEast", Turn::Compass(CompassDirection::SouthEast))]
    #[case("Compass:South", Turn::Compass(CompassDirection::South))]
    #[case("Compass:SouthWest", Turn::Compass(CompassDirection::SouthWest))]
    #[case("Compass:West", Turn::Compass(CompassDirection::West))]
    #[case("Compass:NorthWest", Turn::Compass(CompassDirection::NorthWest))]
    fn test_parse_turn(#[case] input: &str, #[case] turn:Turn) {
        let actual = input.parse::<Turn>();
        assert_eq!(turn, actual.unwrap());
    }

    #[rstest]
    #[case("Count:1", TurnMultiplicity::Count(1))]
    #[case("Always", TurnMultiplicity::Always)]
    fn test_parse_turn_multiplicity(#[case] input: &str, #[case] value:TurnMultiplicity) {
        let actual: TurnMultiplicity = input.parse().unwrap();
        assert_eq!(value, actual);
    }

    #[rstest]
    #[case("Relative:Straight Count:1", TurningPattern { turn:Turn::Relative(TurnDirection::Straight), count:TurnMultiplicity::Count(1) } )]
    #[case("Compass:North Count:1", TurningPattern { turn:Turn::Compass(CompassDirection::North), count:TurnMultiplicity::Count(1) } )]
    #[case("Exit:1 Count:1", TurningPattern { turn:Turn::Exit(1), count:TurnMultiplicity::Count(1) } )]
    #[case("Heading:90 Count:1", TurningPattern { turn:Turn::Heading(90), count:TurnMultiplicity::Count(1) } )]
    fn test_parse_turning_pattern(#[case] input: &str, #[case] value:TurningPattern) {
        let actual : TurningPattern = input.parse().unwrap();
        assert_eq!(value, value);
    }
    #[rstest]
    #[case("data/tests/LoadFromDB/onelink.db", 2)]
    fn test_spanning_tree_num_nodes(#[case] dbfile: &str, #[case] num_nodes:usize) {
        let connection = Connection::open(dbfile).unwrap_or_else(|e| panic!("failed to open {}: {}", dbfile, e));
        let network = Network::from(&connection);
        assert_eq!(num_nodes, network.spanning_tree.deref().borrow().num_nodes());
    }

    #[rstest]
    #[case("data/tests/LoadFromDB/onelink.db", 1, 2, 0)]
    #[case("data/tests/LoadFromDB/twolinks.db", 2, 3, 0)]
    fn test_find_exit(#[case] dbfile:&str, #[case] from_id:u32, #[case] to_id:u32, #[case]exit_index:usize) {
        let connection = Connection::open(dbfile).unwrap_or_else(|e| panic!("failed to open {}: {}", dbfile, e));
        let network = Network::from(&connection);
        let from = &network.get_junc(from_id).borrow().clone();
        let to = &network.get_junc(to_id).borrow().clone();
        let actual = network.find_exit(from, to);
        assert_eq!(exit_index, actual);
    }

    #[rstest]
    #[case("data/tests/LoadFromDB/twolinks.db", 2, 0, 0)]
    #[case("data/tests/LoadFromDB/crossroads.db", 2, 0, 0)]
    #[case("data/tests/LoadFromDB/crossroads.db", 2, 90, 1)]
    #[case("data/tests/LoadFromDB/crossroads.db", 2, 180, 2)]
    #[case("data/tests/LoadFromDB/crossroads.db", 2, 270, 3)]
    fn test_find_exit_by_heading(#[case] dbfile:&str, #[case] to_id:u32, #[case] exit_heading:u32, #[case] exit_index:usize) {
        let connection = Connection::open(dbfile).unwrap_or_else(|e| panic!("failed to open {}: {}", dbfile, e));
        let network = Network::from(&connection);
        let to = &network.get_junc(to_id).borrow().clone();

        let actual = network.find_exit_by_heading(to, exit_heading);
        assert_eq!(exit_index, actual);
    }

    #[rstest]
    #[case(0.0, 180.0)]
    #[case(90.0, 270.0)]
    #[case(180.0, 0.0)]
    #[case(270.0, 90.0)]
    fn test_find_reciprocal_heading(#[case] heading:f64, #[case] reciprocal:f64) {
        assert_eq!(reciprocal, find_reciprocal_heading(heading));
    }

    #[rstest]
    #[case("data/tests/LoadFromDB/crossroads.db", 2, 0.0, 2)]
    #[case("data/tests/LoadFromDB/crossroads.db", 2, 10.0, 2)]
    #[case("data/tests/LoadFromDB/crossroads.db", 2, 45.0, 2)]
    #[case("data/tests/LoadFromDB/crossroads.db", 2, 180.0, 0)]
    #[case("data/tests/LoadFromDB/crossroads.db", 2, 270.0, 1)]
    #[case("data/tests/LoadFromDB/crossroads.db", 2, 90.0, 3)]
    fn test_find_closest_entry(#[case] dbfile: &str, #[case] junc_id:u32, #[case] heading: f64, #[case] exit_index:usize) {
        let connection = Connection::open(dbfile).unwrap_or_else(|e| panic!("failed to open {}: {}", dbfile, e));
        let network = Network::from(&connection);
        let junc = &network.get_junc(junc_id).borrow().clone();
        assert_eq!(exit_index, junc.find_entry(heading))
    }

    #[rstest]
    #[case("data/tests/LoadFromDB/twolinks.db", 2, CompassDirection::North, 0)]
    #[case("data/tests/LoadFromDB/crossroads.db", 2, CompassDirection::North, 0)]
    #[case("data/tests/LoadFromDB/crossroads.db", 2, CompassDirection::NorthEast, 3)]
    // Because we start at exit 0, North and iterate CCW round the exits.
    #[case("data/tests/LoadFromDB/crossroads.db", 2, CompassDirection::East, 3)]
    #[case("data/tests/LoadFromDB/crossroads.db", 2, CompassDirection::West, 1)]
    #[case("data/tests/LoadFromDB/crossroads.db", 2, CompassDirection::South, 2)]
    #[case("data/tests/LoadFromDB/yjunction.db", 2, CompassDirection::North, 0)]
    #[case("data/tests/LoadFromDB/yjunction.db", 2, CompassDirection::NorthEast, 2)]
    #[case("data/tests/LoadFromDB/yjunction.db", 2, CompassDirection::East, 2)]
    fn test_find_exit_from_compass(#[case] dbfile: &str, #[case] junc_id:u32, #[case] dir:CompassDirection, #[case] exit_index:usize) {
        let connection = Connection::open(dbfile).unwrap_or_else(|e| panic!("failed to open {}: {}", dbfile, e));
        let network = Network::from(&connection);
        let junc = &network.get_junc(junc_id).borrow().clone();
        assert_eq!(exit_index, junc.find_exit_from_compass(dir));
    }

    #[rstest]
    #[case("data/tests/LoadFromDB/twolinks.db", 2, 1, 1, 0)]
    #[case("data/tests/LoadFromDB/crossroads.db", 2, 2, 1, 1)]
    #[case("data/tests/LoadFromDB/crossroads.db", 2, 2, 2, 0)]
    #[case("data/tests/LoadFromDB/crossroads.db", 2, 3, 2, 1)]
    #[case("data/tests/LoadFromDB/yjunction.db", 2, 1, 1, 0)]
    #[case("data/tests/LoadFromDB/yjunction.db", 2, 1, 2, 2)]
    fn test_relative_exit(#[case] dbfile:&str, #[case] junc_id:u32, #[case] entry_index:usize, #[case] relative_exit:usize, #[case] exit_index:usize) {
        let connection = Connection::open(dbfile).unwrap_or_else(|e| panic!("failed to open {}: {}", dbfile, e));
        let network = Network::from(&connection);
        let junc = &network.get_junc(junc_id).borrow().clone();
        assert_eq!(exit_index, junc.find_relative_exit(entry_index, relative_exit));
    }

    #[rstest]
    #[case("data/tests/LoadFromDB/twolinks.db", 2, 1, TurnDirection::Straight, 0)]
    #[case("data/tests/LoadFromDB/twolinks.db", 2, 0, TurnDirection::Straight, 1)]
    #[case("data/tests/LoadFromDB/crossroads.db", 2, 2, TurnDirection::Straight, 0)]
    #[case("data/tests/LoadFromDB/crossroads.db", 2, 2, TurnDirection::Left, 1)]
    #[case("data/tests/LoadFromDB/crossroads.db", 2, 0, TurnDirection::Left, 3)]
    #[case("data/tests/LoadFromDB/crossroads.db", 2, 1, TurnDirection::Straight, 3)]
    #[case("data/tests/LoadFromDB/crossroads.db", 2, 3, TurnDirection::Right, 0)]
    #[case("data/tests/LoadFromDB/crossroads.db", 2, 2, TurnDirection::UTurn, 2)]
    #[case("data/tests/LoadFromDB/crossroads.db", 2, 1, TurnDirection::UTurn, 1)]
    #[case("data/tests/LoadFromDB/crossroads.db", 2, 0, TurnDirection::UTurn, 0)]
    #[case("data/tests/LoadFromDB/crossroads.db", 2, 3, TurnDirection::UTurn, 3)]
    #[case("data/tests/LoadFromDB/yjunction.db", 2, 1, TurnDirection::Straight, 0)]
    #[case("data/tests/LoadFromDB/yjunction.db", 2, 1, TurnDirection::Right, 2)]
    fn test_find_exit_from_turn_direction(#[case] dbfile:&str, #[case] junc_id:u32, #[case] entry_index:usize, #[case] turn_dir:TurnDirection, #[case] exit_index:usize) {
        let connection = Connection::open(dbfile).unwrap_or_else(|e| panic!("failed to open {}: {}", dbfile, e));
        let network = Network::from(&connection);
        let junc = &network.get_junc(junc_id).borrow().clone();
        assert_eq!(exit_index, junc.find_exit_from_turn_direction(entry_index, turn_dir));
    }

    #[rstest]
    #[case(0, 0)]
    #[case(45, 0)]
    #[case(45, 0)]
    #[case(180, 1)]
    #[case(270, 0)]
    #[case(360, 0)]
    #[case(90, 1)]
    fn test_hemisphere(#[case] angle: u32, #[case] hemi:u32) {
        assert_eq!(hemi, hemisphere(angle))
    }

    #[rstest]
    #[case("data/tests/LoadFromDB/onelink.db", 1, 0.0)]
    #[case("data/tests/LoadFromDB/yjunction.db", 3, 315.0)]
    #[case("data/tests/LoadFromDB/fivelinks.db", 4, 90.0)]
    #[case("data/tests/LoadFromDB/fivelinks.db", 5, 270.0)]
    fn test_first_segment_for_link(#[case] dbfile:&str, #[case] link_id:u16, #[case] heading:f64) {
        let connection = Connection::open(dbfile).unwrap_or_else(|e| panic!("failed to open {}: {}", dbfile, e));
        let network = Network::from(&connection);
        assert_eq!(heading, network.first_segment_for_link(network.get_link(link_id)).unwrap().h);
    }

    #[rstest]
    #[case("data/tests/LoadFromDB/onelink.db", 1, 0.0)]
    #[case("data/tests/LoadFromDB/yjunction.db", 3, 315.0)]
    #[case("data/tests/LoadFromDB/fivelinks.db", 4, 90.0)]
    #[case("data/tests/LoadFromDB/fivelinks.db", 5, 270.0)]
    fn test_last_segment_for_link(#[case] dbfile:&str, #[case] link_id:u16, #[case] heading:f64) {
        let connection = Connection::open(dbfile).unwrap_or_else(|e| panic!("failed to open {}: {}", dbfile, e));
        let network = Network::from(&connection);
        assert_eq!(heading, network.last_segment_for_link(network.get_link(link_id)).unwrap().h);
    }
}
