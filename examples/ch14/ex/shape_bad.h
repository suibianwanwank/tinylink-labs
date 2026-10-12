// shape_bad.h（改坏版）：在 area 前面加了一个虚函数 draw，只声明，没有人调用它
struct Shape {
#ifdef PURE
    virtual void draw() const = 0;
#else
    virtual void draw() const;
#endif
    virtual double area() const;
    virtual const char *name() const { return "shape"; }
    virtual ~Shape() {}
};
