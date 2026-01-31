A toy road network in Rust as a vehicle for learning and trying out ideas.

![Build Status](https://github.com/triblatron/lrn/actions/workflows/rust.yml/badge.svg)

# Progress

* Road network with nodes and links
* Configuration from a sqlite database using rusqlite
* Depth-first traversal with callbacks for nodes and links
* Building of routing information at each junction
* Parsing of high-level routes such as "1 -1.825 200.0 Relative:Straight Count:1" which means "start at link 1, offset -1.825, distance 200.0, go straight ahead at the junction"
* Parsing of relative (Straight, Left, Right), exit (u8), compass (North, NorthEast, ...), heading (u32) turns.
* Exits numbering inspired by airport runway designations
* Evaluation of reciprocal heading: 0 -> 180, 90 -> 270 etc. with normalisation of output to [0,360]
* Normalisation of heading: -90 -> 270 etc.

# Next up

* More complex networks with more links and a full crossroads junction, including arbitrary headings to test routing decisions.
* Evaluating a route at a junction to determine which exit to take
  * Map relative turn to a heading based on available exits
  * Map compass and heading to nearest exit based on heading of link leading from it

