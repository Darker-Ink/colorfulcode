#include <cstdio>
#include <cuda_runtime.h>

#define BLOCK_SIZE 256
#define CHECK(call) do { cudaError_t e = (call); if (e != cudaSuccess) return 1; } while (0)

// Adds two vectors element by element.
__global__ void vectorAdd(const float* __restrict__ a, const float* b, float* out, int n) {
    int i = blockIdx.x * blockDim.x + threadIdx.x;
    if (i < n) {
        out[i] = a[i] + b[i];
    }
}

__device__ float square(float x) { return x * x; }

template <typename T>
__global__ void reduce(const T* in, T* out, int n) {
    __shared__ T cache[BLOCK_SIZE];
    int tid = threadIdx.x;
    cache[tid] = tid < n ? in[tid] : T(0);
    __syncthreads();
    for (int s = blockDim.x / 2; s > 0; s >>= 1) {
        if (tid < s) cache[tid] += cache[tid + s];
        __syncthreads();
    }
    if (tid == 0) atomicAdd(out, cache[0]);
}

int main() {
    const int n = 1 << 20;
    float *a, *b, *out;
    CHECK(cudaMalloc(&a, n * sizeof(float)));
    CHECK(cudaMalloc(&b, n * sizeof(float)));
    CHECK(cudaMalloc(&out, n * sizeof(float)));
    dim3 grid((n + BLOCK_SIZE - 1) / BLOCK_SIZE);
    vectorAdd<<<grid, BLOCK_SIZE>>>(a, b, out, n);
    cudaDeviceSynchronize();
    printf("done: %d elements, %.2f\n", n, 1.5f);
    cudaFree(a);
    return 0;
}
