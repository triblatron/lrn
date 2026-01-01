mod math;

pub struct RoadID {
    major:i16,
    minor:i16,
}
pub struct Road {
    road_id:RoadID
}

impl RoadID {
    pub fn new(major:i16, minor:i16) -> RoadID {
        RoadID { major, minor }
    }

    pub fn get_major(&self) -> i16 {
        self.major
    }

    pub fn get_minor(&self) -> i16 {
        self.minor
    }
}

impl Road {
    pub fn new(major:i16,minor:i16) -> Road {
        Road {road_id: RoadID{major,minor}}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        let sut = Road::new(1,1);
        assert_eq!(sut.road_id.major, 1);
        assert_eq!(sut.road_id.minor, 1);
    }
}
