package proto

// SearchRequest is the request message for the Search RPC.
type SearchRequest struct {
	Database   string `protobuf:"bytes,1,opt,name=database,proto3" json:"database,omitempty"`
	Collection string `protobuf:"bytes,2,opt,name=collection,proto3" json:"collection,omitempty"`
	Query      string `protobuf:"bytes,3,opt,name=query,proto3" json:"query,omitempty"`
	Limit      int64  `protobuf:"varint,4,opt,name=limit,proto3" json:"limit,omitempty"`
}

func (x *SearchRequest) Reset()         {}
func (x *SearchRequest) String() string { return x.Query }
func (x *SearchRequest) ProtoMessage()  {}

// SearchResponse is the response message for the Search RPC.
type SearchResponse struct {
	Documents []*Document `protobuf:"bytes,1,rep,name=documents,proto3" json:"documents,omitempty"`
	Method    string      `protobuf:"bytes,2,opt,name=method,proto3" json:"method,omitempty"`
	Total     int64       `protobuf:"varint,3,opt,name=total,proto3" json:"total,omitempty"`
}

func (x *SearchResponse) Reset()         {}
func (x *SearchResponse) String() string { return x.Method }
func (x *SearchResponse) ProtoMessage()  {}
