package main

/*
#include <stdint.h>
#include <stdlib.h>

typedef const char cchar_t;
*/
import "C"
import (
	"sync"
	"unsafe"

	"github.com/ProtonMail/go-proton-api/server"
)

type AllocMap[T any] struct {
	sync      sync.RWMutex
	instances []*T
}

func (a *AllocMap[T]) alloc(i *T) int {
	a.sync.Lock()
	defer a.sync.Unlock()

	a.instances = append(a.instances, i)

	return len(a.instances) - 1
}

func (a *AllocMap[T]) free(i int) {
	a.sync.Lock()
	defer a.sync.Unlock()

	a.instances[i] = nil
}

func (a *AllocMap[T]) resolve(i int) *T {
	a.sync.RLock()
	defer a.sync.RUnlock()

	return a.instances[i]
}

var alloc struct {
    AllocMap[server.Server]
}

func init() {
    alloc.sync.Lock();
    alloc.instances = make([]*server.Server, 0, 20)
    alloc.sync.Unlock();
}



//export gpaServerNew
func gpaServerNew() int {
	handle := alloc.alloc(server.New(server.WithTLS(false)))

	return handle
}

//export gpaServerUrl
func gpaServerUrl(h int) *C.char {
	srv := alloc.resolve(h)
	if srv == nil {
		return nil
	}

	url := srv.GetHostURL()
	return C.CString(url)
}

//export gpaServerDelete
func gpaServerDelete(h int) int {
	srv := alloc.resolve(h)
	if srv == nil {
		return -1
	}

	srv.Close()
	alloc.free(h)
	return 0
}

//export gpaCreateUser
func gpaCreateUser(h int, cuser *C.cchar_t, cpassword *C.cchar_t, outUserID **C.char, outAddrID **C.char) int {
	user := C.GoString(cuser)
	password := []byte(C.GoString(cpassword))
	srv := alloc.resolve(h)
	if srv == nil {
		return -1
	}

	userID, addrID, err := srv.CreateUser(user, password)
	if err != nil {
		return -1
	}

	*outUserID= C.CString(userID)
	*outAddrID = C.CString(addrID)

	return 0
}

//export CStrFree
func CStrFree(ptr *C.char) {
    C.free(unsafe.Pointer(ptr))
}

func main() {}
