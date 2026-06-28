use std::any::{Any, TypeId};
use std::collections::{HashMap, HashSet};
use std::hash::Hash;
use std::marker::PhantomData;
use std::sync::OnceLock;
use crate::goof::Goof;

// Note that just because we allow the player to make their own buildings doesn't mean this is broken.
// Essentially each layer of the game has its own specific building that refers to all "simulated" buildings,
// and is differentiated using an internal id system.
static OBJECT_TO_ID: OnceLock<ObjectToID> = OnceLock::new();
static TAG_TO_IDS: OnceLock<HashMap<ObjectTag, HashSet<ObjectID>>> = OnceLock::new();
static ID_TO_TAGS: OnceLock<HashMap<ObjectID, HashSet<ObjectTag>>> = OnceLock::new();

struct ObjectToID {

    data: HashMap<TypeId, ObjectID>,
}

impl ObjectToID {

    fn get<Type: Object>(&self) -> Result<ObjectID, Goof> {

        self.data.get(&TypeId::of::<Type>()).copied().ok_or(Goof::NoAssociatedIDForObject)
    }
}

trait Object: Any {

    fn associated_tags(&self) -> Vec<String>;

    fn as_any(&self) -> &dyn Any;
    fn as_mut_any(&mut self) -> &mut dyn Any;

}

trait Building: Object {

    type Metric: Position;

    fn hitbox(&self) -> Box<dyn Shape<Metric = Self::Metric>>;
    fn position(&self) -> Box<Self::Metric>;

}

// A machine emits this, requesting positions for IO entities.
// The surface returns it back, filled with PortRefs and DockRefs
struct IOManifest<Metric: Position> {

    port_positions: Vec<Metric>,
    ports: Vec<PortRef>,

    dock_positions: Vec<Metric>,
    docks: Vec<DockRef>,
}

trait Machine: Building {

    fn request_io(&self) -> IOManifest<Self::Metric>;
    fn give_io(&mut self, filled_manifest: IOManifest<Self::Metric>);

    fn tick(&mut self, ports: &mut HashMap<PortRef, Port>, docks: &mut HashMap<DockRef, Dock<dyn Object>>);
}

#[derive(Eq, PartialEq, Hash)]
struct PortRef {

    id: u64,
}

struct Port {

    mode: IOMode,

    buffer: Buffer,
}

enum IOMode {

    INPUT,
    OUTPUT,
    NONE,
    BOTH,
}

struct Buffer {

    max_quantity: f64,

    quantity: f64,
    next_quantity: f64,

    object_id: ObjectID,
    next_object_id: ObjectID,
}

struct DockRef {

    id: u64,
}

// Docks work with discrete objects like buildings that can't be abstracted away into basic numbers.
// If the dock is in output mode, a machine will put products into the next_product of the dock, and
// the dock will push out ready_products.
// If the dock is in input mode, the dock will receive products into next_product, and a machine will
// consume ready_products.
// At the end of a tick during the resolve() step, next_product will be moved into ready_product if possible.
struct Dock<Product: Object + ?Sized> {

    mode: IOMode,

    ready_product: Box<Product>,
    next_product: Box<Product>,
}

struct BuildingReference<Underlying: Building> {

    index: u64,
    _marker: PhantomData<Underlying>,
}

struct ObjectTag {

    id: u64
}

#[derive(Copy, Clone)]
struct ObjectID {

    id: u64
}

trait Surface {

    type Metric: Position;

    fn tick(&mut self) -> Result<(), Goof>;
    fn resolve(&mut self) -> Result<(), Goof>;

    fn add_building(&mut self, building: impl Building<Metric = Self::Metric>) -> Result<(), Goof>;
    fn remove(&mut self, position: Self::Metric) -> Result<(), Goof>;
}

struct Space<SpaceMetric: Position> {

    next_id: u64,

    id_to_building: IdToBuilding<SpaceMetric>,
    position_to_id: PositionToId<SpaceMetric>,

    ports: HashMap<PortRef, Port>,
    docks: HashMap<DockRef, Dock<dyn Object>>,
}

struct IdToBuilding<Metric: Position> {

    data: HashMap<u64, Box<dyn Building<Metric=Metric>>>,
}
impl<Metric: Position> IdToBuilding<Metric> {

    fn get(&self, key: u64) -> Result<&Box<dyn Building<Metric=Metric>>, Goof> { self.data.get(&key).ok_or(Goof::IdNotFound) }
    fn get_mut(&mut self, key: u64) -> Result<&mut Box<dyn Building<Metric=Metric>>, Goof> { self.data.get_mut(&key).ok_or(Goof::IdNotFound) }
}

struct PositionToId<Metric: Position> {

    data: HashMap<Metric, u64>,
}
impl<Metric: Position> PositionToId<Metric> {

    fn get(&self, key: &Metric) -> Result<u64, Goof> { self.data.get(key).ok_or(Goof::EmptyPosition).copied() }
}

impl<SpaceMetric: Position + 'static> Space<SpaceMetric> {

    fn get_new_id(&mut self) -> u64 {

        let id = self.next_id;
        self.next_id += 1;
        id
    }

    fn get<Underlying: 'static + Building>(&self, reference: BuildingReference<Underlying>) -> Result<&Underlying, Goof> {

        self.id_to_building.get(reference.index)?.as_any().downcast_ref::<Underlying>().ok_or(Goof::FailedDowncast)
    }

    fn get_mut<Underlying: 'static + Building>(&mut self, reference: BuildingReference<Underlying>) -> Result<&mut Underlying, Goof> {

        self.id_to_building.get_mut(reference.index)?.as_mut_any().downcast_mut::<Underlying>().ok_or(Goof::FailedDowncast)
    }

    fn get_position(&self, position: SpaceMetric) -> Result<& Box<dyn Building<Metric = SpaceMetric>>, Goof> {

        self.id_to_building.get(self.position_to_id.get(&position)?)
    }

    fn get_position_mut(&mut self, position: SpaceMetric) -> Result<&mut Box<dyn Building<Metric=SpaceMetric>>, Goof> {

        self.id_to_building.get_mut(self.position_to_id.get(&position)?)
    }

    fn tick(&mut self) -> Result<(), Goof> {

        self.id_to_building.data.iter_mut().for_each(|(_, building)| {

            if let Some(machine) = building.as_mut_any().downcast_mut::<Box<dyn Machine<Metric=SpaceMetric>>>() {

                machine.tick(&mut self.ports, &mut self.docks);
            }
        });

        Ok(())
    }

    fn add_building(&mut self, mut building: Box<dyn Building<Metric = SpaceMetric>>) -> Result<(), Goof> {

        let hitbox = building.hitbox();

        if hitbox.iter().any(|position: SpaceMetric| -> bool {

            self.position_to_id.get(&position).is_ok()

        }) { return Err(Goof::CollisionOnPlacement) }

        if let Some(machine) = building.as_mut_any().downcast_mut::<Box<dyn Machine<Metric=SpaceMetric>>>() {

            let io_manifest = machine.request_io();

            if io_manifest.port_positions.iter().any(|position| { !hitbox.contains(position) }) ||
                io_manifest.dock_positions.iter().any(|position| { !hitbox.contains(position) })
            { return Err(Goof::IOOutsideBuildingHitbox) }

            io_manifest.port_positions.iter().for_each(|position| {

                let port_ref = PortRef { id: self.get_new_id() };



            })
        }

        Ok(())
    }
}

trait Position: Eq + PartialEq + Hash + Sized {

    fn acceptable_tags(&self) -> &Vec<ObjectTag>;
}

trait Shape {

    type Metric: Position;

    fn contains(&self, position: &Self::Metric) -> bool;
    fn iter(&self) -> Box<dyn Iterator<Item = Self::Metric>>;
}

trait LogisticsSystem {

    fn claim_ports(&self) -> (Vec<ObjectTag>, Vec<ObjectID>); // Used to claim what tags and items this logistics system operates over
    fn claim_buildings(&self) -> (Vec<ObjectTag>, Vec<ObjectID>); // Used to claim what buildings this logistics system needs to be notified about the placing and breaking of

    fn compile(&mut self) -> &mut Vec<&mut BuildingReference<impl Building>>;

}