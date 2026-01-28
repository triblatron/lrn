use std::cell::{RefCell};
use std::collections::HashSet;
use rusqlite::{Connection, Result, Error, Row};

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

pub struct Exit {
    link_id: u16,
    exit: u32
}

pub struct Junction {
    id:u32,
    incoming : Vec<Exit>,
    outgoing : Vec<Exit>
}

impl Junction {
    pub fn new(id:u32) -> Junction {
        Junction {
            id,
            incoming:Vec::new(),
            outgoing:Vec::new()
        }
    }

    fn build_routes(&self, network:& Network, routing:&mut Routing) -> () {
        // Build immediately accessible hops
        // for exit in &self.outgoing {
        //     routing.hops.insert(Hop::from(self.id,
        //                                 LogicalAddress::new(Identifier::new(exit.link_id, 0, 0, 0), Mask::new(true,false,false,false)),
        //                                 LogicalAddress::new(Identifier::new(exit.link_id, 0, 0,0), Mask::new(true,false,false,false)), exit.exit));
        // }
        // for exit in &self.incoming {
        //     routing.hops.insert(Hop::from(self.id,
        //                                 LogicalAddress::new(Identifier::new(exit.link_id, 0, 0, 0), Mask::new(true,false,false,false)),
        //                                 LogicalAddress::new(Identifier::new(exit.link_id, 0, 0,0), Mask::new(true,false,false,false)), exit.exit));
        // }
        //
        // let mut reciprocals: HashSet<Hop> = HashSet::new();
        // for hop in &routing.hops {
        //     // Look at the incoming links and add a hop for the destination
        //     // if let Some(origin) = network.get_link(hop.destination.id.link).origin {
        //     //     // Add a reciprocal route
        //     //     for incoming in &network.get_junc(origin).incoming {
        //     //         reciprocals.insert(Hop::from(origin,
        //     //                                    LogicalAddress::new(Identifier::new(hop.destination.id.link, 0, 0, 0), Mask::new(true,false,false,false)),
        //     //                                    LogicalAddress::new(Identifier::new(*incoming, 0, 0, 0), Mask::new(true, false, false, false)), 90));
        //     //
        //     //     }
        //     //     // for outgoing in &network.get_junc(origin).outgoing {
        //     //     //     reciprocals.insert(Hop::from(origin,
        //     //     //                                 LogicalAddress::new(Identifier::new())))
        //     //     // }
        //     // }
        //     for outgoing in &network.get_junc(hop.junction).outgoing {
        //         let link = network.get_link(hop.destination.id.link);
        //         if let Some(origin) = link.origin {
        //             let mut found = false;
        //             for hop2 in &routing.hops {
        //                 if hop2.junction == origin && hop2.destination.id.link == outgoing.link_id {
        //                     found = true;
        //                 }
        //             }
        //             if !found {
        //                 reciprocals.insert(Hop::from(origin,
        //                                              LogicalAddress::new(Identifier::new(outgoing.link_id, 0, 0, 0), Mask::new(true, false, false, false)),
        //                                              hop.destination, outgoing.exit));
        //             }
        //         }
        //     }
        // }
        // routing.hops = &routing.hops|&reciprocals;
    }

    fn from_query(id:u32) -> Junction {
        Junction {
            id,
            incoming:Vec::new(),
            outgoing:Vec::new()
        }
    }

    pub fn num_outgoing(&self) -> usize {
        self.outgoing.len()
    }

    pub fn add_outgoing(&mut self, id:u16, exit_id:u32) {
        self.outgoing.push(Exit{link_id:id,exit:exit_id});
    }
    pub fn num_incoming(&self) -> usize {
        self.incoming.len()
    }

    pub fn add_incoming(&mut self, id:u16, exit_id:u32) {
        self.incoming.push(Exit{link_id:id,exit:exit_id});
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
pub struct Network {
    links : Vec<Box<Link>>,
    junctions : Vec<Box<Junction>>,
    tiles: Vec<Box<Tile>>,
    segments: Vec<Box<Segment>>,
    // One for each Junction
    routing: RefCell<Routing>
}

impl<'a> Network {
    pub fn new(links:Vec<Box<Link>>, junctions:Vec<Box<Junction>>) -> Network {
        Network {
            links,
            junctions,
            tiles: Vec::new(),
            segments: Vec::new(),
            routing:RefCell::new(Routing::new())
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
        network.set_junction_connections(&mut junc_gw.find_connections().unwrap_or(Vec::<(u32,u16,bool,u32)>::new()));
        network.set_tiles(tile_gw.find_all().unwrap_or(Vec::new()));
        network.set_segments(seg_gw.find_all().unwrap_or(Vec::new()));
        network.build_routes();
        network
    }

    fn dummy(&self, junc:&Junction, link:&Link, exit:u32, dest_junc:u32) -> () {
        println!("{} {} {} {}", junc.id, link.id, exit, dest_junc);
    }

    fn build_routes_for_junction(&self, _:&Junction) -> () {

    }

    fn build_routes(&mut self) {
        for junc in &self.junctions {
            junc.build_routes(self, &mut self.routing.borrow_mut());
        }
        let print_step = |junc:&Junction, link:&Link, exit:u32, dest_junc:u32, path:&Vec<(u32,u32)>| {
            // self.routing.borrow_mut().hops.insert(Hop::from(junc.id,
            //                                                 LogicalAddress::new(Identifier::new(link.id, 0, 0, 0), Mask::new(true, false, false, false)),
            //                                                 LogicalAddress::new(Identifier::new(link.id, 0, 0, 0), Mask::new(true, false, false, false)),
            //                                                 exit
            // )
            // );
            // For each outgoing link reachable directly from dest_junc, add a route from origin to origin via link
            //let dest_junc = self.get_junc(dest_junc);
            // for outgoing_exit in &dest_junc.outgoing {
            //     let outgoing_link = self.get_link(outgoing_exit.link_id);
            //     self.routing.borrow_mut().hops.insert(Hop::from(junc.id,
            //     LogicalAddress::new(Identifier::new(outgoing_link.id, 0, 0, 0), Mask::new(true, false, false, false)),
            //     LogicalAddress::new(Identifier::new(link.id, 0, 0, 0), Mask::new(true, false, false, false)),
            //     exit
            //     ));
            //     println!("Add route: {} {} {} {}", junc.id, outgoing_exit.link_id, link.id, exit);
            // }
            if let Some(last_junc) = path.last() {
                let last_junc = self.get_junc(last_junc.0);

                if last_junc.outgoing.is_empty() {
                    // Iterate over path, adding routes
                    for i in 0..path.len() {
                        let src_junc = self.get_junc(path[i].0);
                        for j in i+1..path.len() {
                            let dest_junc = self.get_junc(path[j].0);
                            println!("origin_junc: {} dest_junc: {} exit {}", src_junc.id, dest_junc.id, exit);
                            println!("Add route {} {} {}", src_junc.id, dest_junc.id, path[i].1);
                            self.routing.borrow_mut().hops.insert(Hop::from(src_junc.id, dest_junc.id, path[i].1));
                        }
                    }
                }
            }
        };
        self.depth_first_traversal(&print_step, |junc:&Junction| println!("{}", junc.id));
    }

    fn depth_first_traversal_helper<LinkFunc, JuncFunc>(& self, junc:&Junction, visited:&mut HashSet<u32>, path: &mut Vec<(u32,u32)>, link_func:&LinkFunc, junc_func:&JuncFunc) -> ()
    where LinkFunc : Fn(&Junction, &Link, u32, u32, &Vec<(u32,u32)>),
        JuncFunc: Fn(&Junction)
    {
        if !visited.contains(&junc.id) {
            visited.insert(junc.id);
            for exit in &junc.incoming {
                let link = self.get_link(exit.link_id);
                path.push((junc.id,exit.exit));
                junc_func(junc);
                let dest_junc = link.destination;
                if let Some(origin) = link.origin && dest_junc.is_some() {
                    link_func(junc, link, 270, origin, path);
                    if !visited.contains(&origin) {
                        self.depth_first_traversal_helper(self.get_junc(origin), visited, path, link_func, junc_func);
                    }
                }
            }

            for exit in &junc.outgoing {
                let link = self.get_link(exit.link_id);
                if let Some(destination) = link.destination {
                    path.push((junc.id,exit.exit));
                    link_func(junc, link, 90, destination, path);
                    if !visited.contains(&destination) {
                        self.depth_first_traversal_helper(self.get_junc(destination), visited, path, link_func, junc_func);
                    }
                }
            }
            path.pop();
        }
    }

    pub fn depth_first_traversal<LinkFunc, JuncFunc>(&self, link_func:&LinkFunc, junc_func:JuncFunc) -> ()
    where LinkFunc: Fn(&Junction, &Link, u32, u32, &Vec<(u32,u32)>),
        JuncFunc: Fn(&Junction)
    {
        let mut visited: HashSet<u32> = HashSet::new();
        let mut path:Vec<(u32,u32)> = Vec::new();
        if !self.junctions.is_empty() {
            let junc = &self.get_junc(1);
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
        }
    }

    pub fn route(&self, junc_id: u32, src_junc:u32, dest_junc:u32, to_dest:bool) -> Option<Hop> {
        let src_junc = self.get_junc(src_junc);
        // let origin = src_link.origin;
        // let dest = src_link.destination;

        for hop in &self.routing.borrow().hops {
            let junc = self.get_junc(hop.junction);
            let dest = self.get_junc(hop.dest_junc);
            if  junc.id == junc_id && dest.id == dest_junc && to_dest {
                return Some(*hop);
            }
            if junc.id == junc_id && dest.id == src_junc.id && !to_dest {
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

    pub fn set_junctions(&mut self, junctions:Vec<Box<Junction>>) {
        self.junctions = junctions;
    }

    pub fn set_tiles(&mut self, tiles:Vec<Box<Tile>>) {
        self.tiles = tiles;
    }
    pub fn set_junction_connections(&mut self, connections: &mut Vec<(u32, u16, bool, u32)>) {
        for connection in connections {
            if connection.2 {
                self.get_junc_mut(connection.0).add_outgoing(connection.1, connection.3);
            }
            else {
                self.get_junc_mut(connection.0).add_incoming(connection.1, connection.3);
            }
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

    pub fn get_junc_mut(&mut self, id:u32) -> &mut Junction {
        &mut self.junctions[(id - 1) as usize]
    }

    pub fn get_junc(&self, id:u32) -> &Junction {
        &self.junctions[(id-1) as usize]
    }

    pub fn get_junc_if_exists(&self, id: Option<u32>) -> Option<&Junction> {
        if let Some(valid_id) = id {
            Some(self.get_junc(valid_id))
        }
        else {
            None
        }
    }
    pub fn get_junc_if_exists_mut(&mut self, id: Option<u32>) -> Option<&mut Junction> {
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
    junctions:Vec<Box<Junction>>,
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
            j.outgoing.push(Exit{link_id:self.links.last().unwrap().id,exit:90});
        }
    }

    pub fn add_junction(&mut self) {
        self.junctions.push(Box::new(Junction::new(self.next_junc)));
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
    pub fn find_all(&self) -> Result<Vec<Box<Junction>>, Error> {
        let mut statement = self.connection.prepare("SELECT * FROM junctions;");
        if let  Err(e) = statement {
            return Err(e);
        }
        let mut statement = statement.unwrap();
        let junc_iter = statement.query_map([], |row| {
            Ok(Junction::from_query(row.get(0).unwrap()))
        });
        let mut juncs = Vec::new();
        for junc in junc_iter.unwrap() {
            juncs.push(Box::new(junc.unwrap()));
        }
        Ok(juncs)
    }

    pub fn find_connections(&self) -> Result<Vec<(u32,u16,bool,u32)>, Error> {
        let mut statement = self.connection.prepare("SELECT * FROM junctions_links;");
        if let  Err(e) = statement {
            return Err(e);
        }
        let mut statement = statement.unwrap();
        let connection_iter = statement.query_map([], |row| {
            Ok((row.get::<usize, u32>(0).unwrap() as u32, row.get::<usize,u16>(1).unwrap(), row.get::<usize,bool>(2).unwrap(), row.get::<usize,u32>(3).unwrap()))
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
#[cfg(test)]
mod tests {
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
    #[case("data/tests/LoadFromDB/onelink.db", 2, 1, 1, 0)]
    #[case("data/tests/LoadFromDB/onelink.db", 2, 2, 0, 1)]
    #[case("data/tests/LoadFromDB/twolinks.db", 3, 2, 1, 1)]
    #[case("data/tests/LoadFromDB/twolinks.db", 3, 3, 0, 1)]
    fn test_create_network_from_db_junctions(#[case]dbfile:&str, #[case] num_juncs:usize, #[case] junc_id:u32, #[case] num_outgoing:usize, #[case] num_incoming:usize) {
        let connection = Connection::open(dbfile).unwrap_or_else(|e| panic!("failed to open {}: {}", dbfile, e));
        let mut network = Network::from(&connection);
        assert_eq!(num_juncs, network.num_junctions());
        assert_eq!(num_outgoing, network.get_junc_mut(junc_id).num_outgoing());
        assert_eq!(num_incoming, network.get_junc_mut(junc_id).num_incoming());
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
    // #[case("data/tests/LoadFromDB/onelink.db", 1, 1, 1, true, true, 1, 90)]
    #[case("data/tests/LoadFromDB/twolinks.db", 1, 1, 2, true, true, 90)]
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
}
