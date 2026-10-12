#pragma once

// Shared declarations keep every translation unit's type definition identical.
struct BaseInitializer {
    BaseInitializer();
};

extern int base_value;
extern int derived;
