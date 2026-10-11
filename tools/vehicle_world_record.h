/* Authored private wire for a selected single-car serial world comparison. */
typedef struct {
 unsigned steps,parameters[11],correction[5],material[13],rows[16][48];
 unsigned cache[15],ends[4],properties[17],workspace[8],world[11],lifecycle[6],row_refs[16];
} VehicleWorldRecord;
_Static_assert(sizeof(VehicleWorldRecord)==3500,"vehicle world wire size");
