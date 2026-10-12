// shape.h
struct Shape {
    virtual double area() const;        // 第一个非 inline、非纯虚的虚函数：key function
    virtual const char *name() const { return "shape"; }
    virtual ~Shape() {}
};
