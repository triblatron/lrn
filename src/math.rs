use std::rc::{Rc,Weak};
use std::cell::RefCell;
use rstest::rstest;
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
        let mut digits : String = String::new();
        let mut i = 0;
        let mut allow_negative = false;
        for c in str.chars() {
            match state {
                ParsingState::Initial => {
                    if c.is_digit(10) || (c == '-' && allow_negative) {
                        digits.push(c);
                        state = ParsingState::FoundDigit;
                    }
                    else if c == '-' {
                        return Err("Expected whole number, got minus sign");
                    }
                },
                ParsingState::FoundDigit => {
                    if c.is_digit(10) {
                        digits.push(c);
                    }
                    else if c == '.' {
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
                            digits.clear();
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
        }
        if let ParsingState::FoundDigit = state && i==3 {
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

pub struct Segment {
    reference: Vec<Box<Curve>>
}

pub struct Tile {
    segments: Vec<Box<Segment>>
}

pub struct Junction<'a> {
    incoming : Vec<&'a Link<'a>>
}
pub struct Link<'a> {
    tiles: Vec<Box<Tile>>,
    origin: &'a Junction<'a>,
    destination: &'a Junction<'a>
}

pub struct Routing {
    junction: u32,
    destination: LogicalAddress,
    next_hop: LogicalAddress
}

pub struct Network<'a> {
    links : Vec<Box<Link<'a>>>,
    junctions : Vec<Box<Junction<'a>>>,
    routing: Vec<Routing>
}
#[cfg(test)]
mod tests {
    use rstest::rstest;
    use crate::math::{Curve, Identifier, InertialCoord, LogicalAddress, LogicalCoord, Mask};

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
}