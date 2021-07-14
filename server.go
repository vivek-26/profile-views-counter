package main

import (
	"fmt"

	"profile-views-counter/conf"
)

func main() {
	config, err := conf.Load()
	if err != nil {
		panic(err)
	}

	fmt.Println(config)
}
