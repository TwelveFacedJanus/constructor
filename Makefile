TARGET=target/release/constructor
LINUX_DESTINATION=/usr/local/bin/constructor
MACOS_DESTINATION=/opt/homebrew/bin/constructor

linux_install:
	cp -r $(TARGET) $(LINUX_DESTINATION)

macos_install:
	cp -r $(TARGET) $(MACOS_DESTINATION)
