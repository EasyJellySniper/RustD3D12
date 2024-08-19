// simple hello world triangle

// screen uv to form a quad, make 2 triangles in clockwise order
// (0,1) (1,1)
// (0,0) (1,0)
static const float2 GScreenUV[6] =
{
    float2(0, 0),
    float2(0, 1),
    float2(1, 1),
    float2(0, 0),
    float2(1, 1),
    float2(1, 0)
};

// define the screen space triangle position
static const float2 GTrianglePointA = float2(960, 270);
static const float2 GTrianglePointB = float2(480, 810);
static const float2 GTrianglePointC = float2(1440, 810);

// constant that holds a time parameter
uint GTimeMS : register(b0);

void HelloWorldVS(uint VertexID : SV_VertexID, out float4 OutPos : SV_POSITION)
{
    // map UV to NDC [0,1] to [-1,1]
    OutPos.xy = GScreenUV[VertexID];
    OutPos.xy = OutPos.xy * 2 - 1;
    OutPos.zw = float2(0, 1);
}

bool IsInsideTriangle(float2 p, float2 p1, float2 p2, float2 p3)
{
    // calculate the solution of Barycentric coordinate system to tell whether it's inside a triangle (include the cast point on the edge)
    // note this is for testing 2D triangle only!
    float v = ((p2.y - p3.y)*(p.x - p3.x) + (p3.x - p2.x)*(p.y - p3.y)) / ((p2.y - p3.y)*(p1.x - p3.x) + (p3.x - p2.x)*(p1.y - p3.y));
    float w = ((p3.y - p1.y)*(p.x - p3.x) + (p1.x - p3.x)*(p.y - p3.y)) / ((p2.y - p3.y)*(p1.x - p3.x) + (p3.x - p2.x)*(p1.y - p3.y));
    float u = 1.0f - v - w;

    return v >= 0.0f && w >= 0.0f && u >= 0.0f;
}

float4 HelloWorldPS(float4 InPos : SV_POSITION) : SV_TARGET
{
    // clip pixels that are not in triangle area
    float2 ShiftedAmount = float2(800, 0);
    float2 ShiftedPointA = lerp(GTrianglePointA - ShiftedAmount, GTrianglePointA + ShiftedAmount, cos(GTimeMS * 0.001f) * 0.5f + 0.5f);
    if (!IsInsideTriangle(InPos.xy, ShiftedPointA, GTrianglePointB, GTrianglePointC))
    {
        clip(-1);
    }

    // fake light shooting into forward direction from point of view
    float3 FakeLightColor = float3(1 , 0.93f, 0.31f);
    float3 FakeLightDir = float3(0, 0, 1);
    float3 FakeNormal = float3(0, 0, -1);
    float Offset = GTimeMS * 0.005f;
    float PosScale = 0.005f;

    FakeNormal.z = sin(InPos.y * PosScale + Offset) * 0.5f + 0.5f;
    FakeNormal.z = -FakeNormal.z;
    float Intensity = saturate(dot(-FakeLightDir, FakeNormal));

    return float4(Intensity * FakeLightColor, 1.0f);
}