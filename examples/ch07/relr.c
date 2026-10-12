static int v[512];
int *ptrs[512] = {
#define P8(i) &v[i],&v[i+1],&v[i+2],&v[i+3],&v[i+4],&v[i+5],&v[i+6],&v[i+7],
#define P64(i) P8(i)P8(i+8)P8(i+16)P8(i+24)P8(i+32)P8(i+40)P8(i+48)P8(i+56)
P64(0)P64(64)P64(128)P64(192)P64(256)P64(320)P64(384)P64(448)
};
int main(void) { return *ptrs[3] + (ptrs[511] - ptrs[0]) - 511; }
