/* Retained AD7040 -> AD6C40 -> B77310 with supplied relative/world contacts.
   Explicit construction schedule; no collision detection or whole scheduler. */
#define GRID_CONTACT_GROUPS
#define GRID_OWNED_SHAPES
#define GRID_GROUPED_TICK
#define GRID_GEOMETRY_CONTACTS
#define GRID_CARRIED_GEOMETRY
#include "probe_physical_tick.c"
