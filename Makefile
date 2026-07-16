.PHONY: all release debug clean install uninstall deb rpm tar debug-deb debug-rpm debug-tar package debug-package test

all: release

test:
	@echo "Building and running unit tests..."
	cmake -B build-tests -DCMAKE_BUILD_TYPE=Debug -DRLSHIM_BUILD_TESTS=ON -DRLSHIM_BUILD_APP=OFF
	cmake --build build-tests --target rlshim_tests -j$$(nproc)
	ctest --test-dir build-tests --output-on-failure

release: clean
	@echo "Building Release version..."
	cmake -B build -DCMAKE_BUILD_TYPE=Release
	cmake --build build -j$$(nproc)

debug: clean
	@echo "Building Debug version..."
	cmake -B build -DCMAKE_BUILD_TYPE=Debug
	cmake --build build -j$$(nproc)

clean:
	rm -rf build build-tests

install:
	cmake --install build

uninstall:
	sudo cmake --build build --target uninstall

deb: release
	@echo "Building DEB package..."
	cd build && cpack -G DEB

rpm: release
	@echo "Building RPM package..."
	cd build && cpack -G RPM

tar: release
	@echo "Building tar.gz archive..."
	cd build && cpack -G TGZ
	
debug-deb: debug
	@echo "Building DEB package..."
	cd build && cpack -G DEB

debug-rpm: debug
	@echo "Building RPM package..."
	cd build && cpack -G RPM

debug-tar: debug
	@echo "Building tar.gz archive..."
	cd build && cpack -G TGZ

package: deb rpm tar
debug-package: debug-deb debug-rpm debug-tar
