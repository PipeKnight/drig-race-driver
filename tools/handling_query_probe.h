/* Owned bounded receiver for whole original98BFE0. No helper replacement. */
#define HANDLING_QUERY_CAPACITY 64
#define HANDLING_QUERY_BYTES (0x8f0+HANDLING_QUERY_CAPACITY*0x2e0)
typedef struct { unsigned present,count,records[HANDLING_QUERY_CAPACITY][2]; } HandlingQueries;
static unsigned query_receiver(const HandlingQueries *q,unsigned char *receiver){
    if(q->present>1||q->count>HANDLING_QUERY_CAPACITY||(!q->present&&q->count))return 35;
    for(unsigned i=q->count;i<HANDLING_QUERY_CAPACITY;i++)if(q->records[i][0]||q->records[i][1])return 35;
    memset(receiver,0,HANDLING_QUERY_BYTES);put(receiver,0x888,q->count);
    for(unsigned i=0;i<q->count;i++){put(receiver,0xb3c+i*0x2e0,q->records[i][0]);put(receiver,0x948+i*0x2e0,q->records[i][1]);}
    return 0;
}
