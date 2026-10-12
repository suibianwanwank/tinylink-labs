// 1.1 在 area() 前面加了一个虚函数 perimeter()
struct Shape {
#ifdef V2
    virtual int perimeter() const;
#endif
    virtual int area() const;
    virtual ~Shape();
    int side;
};
Shape *make_square(int side);
