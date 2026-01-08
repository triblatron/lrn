use std::arch::aarch64::uint32x4_t;
use std::rc::{Rc, Weak};
use std::cell::RefCell;
use rstest::rstest;
use rusqlite::{Connection, Result, Error, Row};
pub enum ParsingState {
    Initial,
    FoundDigit,
    Accepted
}
// An identifier for a network component
#[derive(PartialEq, Debug)]
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
#[derive(PartialEq,Debug)]
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


#[derive(PartialEq, Debug)]
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
        let mut id = Identifier::parse(id);
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
pub struct Junction {
    id:u32,
    incoming : Vec<u16>,
    outgoing : Vec<u16>
}

impl Junction {
    pub fn new(id:u32) -> Junction {
        Junction {
            id,
            incoming:Vec::new(),
            outgoing:Vec::new()
        }
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

    pub fn add_outgoing(&mut self, id:u16) {
        self.outgoing.push(id);
    }
    pub fn num_incoming(&self) -> usize {
        self.incoming.len()
    }

    pub fn add_incoming(&mut self, id:u16) {
        self.incoming.push(id);
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
pub struct Routing {
    junction: u32,
    destination: LogicalAddress,
    next_hop: LogicalAddress
}

pub struct Network {
    links : Vec<Box<Link>>,
    junctions : Vec<Box<Junction>>,
    tiles: Vec<Box<Tile>>,
    segments: Vec<Box<Segment>>,
    routing: Vec<Routing>
}

impl<'a> Network {
    pub fn new(links:Vec<Box<Link>>, junctions:Vec<Box<Junction>>) -> Network {
        Network {
            links,
            junctions,
            tiles: Vec::new(),
            segments: Vec::new(),
            routing:Vec::new()
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
        network.set_junction_connections(&mut junc_gw.find_connections().unwrap_or(Vec::<(u32,u16,bool)>::new()));
        network.set_tiles(tile_gw.find_all().unwrap_or(Vec::new()));
        network.set_segments(seg_gw.find_all().unwrap_or(Vec::new()));
        network
    }

    pub fn empty() -> Network {
        Network {
            links:Vec::new(),
            junctions:Vec::new(),
            tiles: Vec::new(),
            segments:Vec::new(),
            routing:Vec::new()
        }
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
    pub fn set_junction_connections(&mut self, connections: &mut Vec<(u32, u16, bool)>) {
        for connection in connections {
            if connection.2 {
                self.get_junc(connection.0).add_outgoing(connection.1);
            }
            else {
                self.get_junc(connection.0).add_incoming(connection.1);
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

    pub fn get_junc(&mut self, id:u32) -> &mut Junction {
        &mut self.junctions[(id - 1) as usize]
    }

    pub fn num_tiles(&self) -> usize {
        self.tiles.len()
    }

    pub fn num_segments(&self) -> usize {
        self.segments.len()
    }

    pub fn num_route_info(&self) -> usize {
        self.routing.len()
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
            j.outgoing.push(self.links.last().unwrap().id)
        }
    }

    pub fn add_junction(&mut self) {
        self.junctions.push(Box::new(Junction::new(self.next_junc)));
        self.next_junc += 1;
    }

    pub fn add_straight(&mut self, pos:InertialCoord, length:f64) {

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
        let mut statement = self.connection.prepare("SELECT * FROM links;");
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

    pub fn find_connections(&self) -> Result<Vec<(u32,u16,bool)>> {
        let mut statement = self.connection.prepare("SELECT * FROM junctions_links;");
        if let  Err(e) = statement {
            return Err(e);
        }
        let mut statement = statement.unwrap();
        let connection_iter = statement.query_map([], |row| {
            Ok((row.get::<usize, u32>(0).unwrap() as u32, row.get::<usize,u16>(1).unwrap(), row.get::<usize,bool>(2).unwrap()))
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
        let mut statement = self.connection.prepare("SELECT * FROM tiles;");
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
    pub fn new(connection: &Connection) -> SegmentGateway {
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
    fn test_logical_to_inertial_coords(#[case] offset: f64, #[case] distance: f64, #[case] loft: f64) {
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
        let mut network = Network::from(&connection);
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
        assert_eq!(num_outgoing, network.get_junc(junc_id).num_outgoing());
        assert_eq!(num_incoming, network.get_junc(junc_id).num_incoming());
    }

    #[rstest]
    #[case("data/tests/LoadFromDB/onelink.db", 2, 1)]
    fn test_create_network_from_db_tiles(#[case] dbfile:&str, #[case] num_tiles:usize, #[case] tile_id:u16) {
        let connection = Connection::open(dbfile).unwrap_or_else(|e| panic!("failed to open {}: {}", dbfile, e));
        let mut network = Network::from(&connection);
        assert_eq!(num_tiles, network.num_tiles());
    }

    #[rstest]
    #[case("data/tests/LoadFromDB/onelink.db", 2)]
    fn test_create_network_from_db_segments(#[case] dbfile:&str, #[case] num_segments:usize) {
        let connection = Connection::open(dbfile).unwrap_or_else(|e| panic!("failed to open {}: {}", dbfile, e));
        let mut network = Network::from(&connection);
        assert_eq!(num_segments, network.num_segments());
    }
}