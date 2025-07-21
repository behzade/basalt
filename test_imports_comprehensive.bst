// Test various import scenarios

// Basic import
import Std::Fmt;
Fmt::println("Hello from Std::Fmt");

// Import with alias
import Std::Collections as Collections;
Collections::Vec::new();

// Multiple imports
import Std::String;
import Std::Math;

String::from("test");
Math::add(1, 2);

// Local import (Self::)
import Self::Utils;
Utils::helper_function(); 