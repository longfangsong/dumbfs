prepare:
	touch ./dev.img || :
	mkdir ./mountpoint || :
prepare-run:
	rm -rf ./dev.img
	make prepare
	umount ./mountpoint || :
clean:
	rm -rf ./dev.img
	mkdir ./mountpoint